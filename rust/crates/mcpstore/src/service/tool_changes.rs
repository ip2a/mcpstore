use std::collections::{BTreeMap, BTreeSet};

use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_changed_tools(
        &self,
        instance_id: InstanceId,
        force_refresh: bool,
    ) -> Result<ToolChangeSummary> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        let instance_ids = [instance_id];

        if self.source_mode == SourceMode::Db {
            for instance_id in &instance_ids {
                self.queue_control_request(
                    "ServiceRefreshToolsRequested",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "force_refresh": force_refresh,
                    }),
                )
                .await?;
            }
            let timestamp = chrono::Utc::now().timestamp();
            let total_services = instance_ids.len();
            return Ok(ToolChangeSummary {
                changed: false,
                services: Vec::new(),
                trigger: if force_refresh {
                    "queued_manual_force"
                } else {
                    "queued_manual"
                }
                .to_string(),
                timestamp,
                details: serde_json::json!({
                    "queued": true,
                    "queued_instances": instance_ids,
                    "total_services": total_services,
                    "successful_updates": 0,
                    "failed_updates": 0,
                    "services_with_changes": 0,
                    "total_changes": 0,
                    "service_results": [],
                    "errors": [],
                }),
            });
        }

        let mut results = Vec::with_capacity(instance_ids.len());
        let mut errors = Vec::new();

        for instance_id in instance_ids {
            match self
                .refresh_service_tools_with_diff(instance_id, force_refresh)
                .await
            {
                Ok(result) => results.push(result),
                Err(error) => errors.push(serde_json::json!({
                    "instance_id": instance_id,
                    "error": error.to_string()
                })),
            }
        }

        let changed_instances = results
            .iter()
            .filter(|result| result.changed)
            .map(|result| result.client_id.clone())
            .collect::<Vec<_>>();
        let total_changes = results
            .iter()
            .map(|result| result.changes_count)
            .sum::<usize>();
        let timestamp = chrono::Utc::now().timestamp();

        Ok(ToolChangeSummary {
            changed: !changed_instances.is_empty(),
            services: changed_instances,
            trigger: if force_refresh {
                "manual_force"
            } else {
                "manual"
            }
            .to_string(),
            timestamp,
            details: serde_json::json!({
                "total_services": results.len() + errors.len(),
                "successful_updates": results.len(),
                "failed_updates": errors.len(),
                "services_with_changes": results.iter().filter(|result| result.changed).count(),
                "total_changes": total_changes,
                "service_results": results,
                "errors": errors,
            }),
        })
    }

    pub(crate) async fn refresh_service_tools_with_diff(
        &self,
        instance_id: InstanceId,
        force_refresh: bool,
    ) -> Result<ToolChangeServiceResult> {
        self.refresh_from_db_if_needed().await?;
        let Some(instance) = self.registry.find_instance(instance_id).await else {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        };
        let old_tools = instance.tools;
        let is_openapi = self.is_openapi_virtual_instance(instance_id).await?;

        if force_refresh {
            self.pool.disconnect(instance_id).await.ok();
            self.connect_service_internal(instance_id, false).await?;
        } else if is_openapi || !self.pool.is_connected(instance_id).await {
            self.ensure_instance_connected(instance_id).await?;
        }

        let tool_infos = if is_openapi {
            self.registry.list_instance_tools(instance_id).await
        } else {
            self.pool
                .list_tools(instance_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
        };

        let diff = Self::diff_tool_infos(&old_tools, &tool_infos);
        let mut updated = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        updated.tools = tool_infos;
        updated.status = ConnectionStatus::Connected;
        let service_name = updated.service_name.clone();
        self.registry.register_instance(updated).await;

        let tools = self.registry.list_instance_tools(instance_id).await;
        self.cache_instance_connected(instance_id, &tools).await?;

        let timestamp = chrono::Utc::now().timestamp();
        self.cache
            .put_event(
                "service_instance",
                &format!("{instance_id}:tools_refreshed:{timestamp}"),
                serde_json::json!({
                    "event": "instance_tools_refreshed",
                    "instance_id": instance_id,
                    "service_name": service_name,
                    "timestamp": timestamp,
                    "changes_count": diff.changes_count,
                    "added_tools": diff.added_tools,
                    "removed_tools": diff.removed_tools,
                    "updated_tools": diff.updated_tools,
                }),
            )
            .await?;

        Ok(ToolChangeServiceResult {
            changed: diff.changes_count > 0,
            changes_count: diff.changes_count,
            added_tools: diff.added_tools,
            removed_tools: diff.removed_tools,
            updated_tools: diff.updated_tools,
            service_name,
            client_id: instance_id.to_string(),
            timestamp,
        })
    }

    fn diff_tool_infos(
        old_tools: &[crate::registry::ToolInfo],
        new_tools: &[crate::registry::ToolInfo],
    ) -> ToolDiff {
        let old = old_tools
            .iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect::<BTreeMap<_, _>>();
        let new = new_tools
            .iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect::<BTreeMap<_, _>>();
        let old_names = old.keys().cloned().collect::<BTreeSet<_>>();
        let new_names = new.keys().cloned().collect::<BTreeSet<_>>();

        let added_tools = new_names
            .difference(&old_names)
            .cloned()
            .collect::<Vec<_>>();
        let removed_tools = old_names
            .difference(&new_names)
            .cloned()
            .collect::<Vec<_>>();
        let updated_tools = old_names
            .intersection(&new_names)
            .filter_map(|name| {
                let old_tool = old.get(name)?;
                let new_tool = new.get(name)?;
                if old_tool.description != new_tool.description
                    || old_tool.input_schema != new_tool.input_schema
                    || old_tool.output_schema != new_tool.output_schema
                {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        ToolDiff {
            changes_count: added_tools.len() + removed_tools.len() + updated_tools.len(),
            added_tools,
            removed_tools,
            updated_tools,
        }
    }

    #[cfg(test)]
    pub(crate) fn diff_tool_infos_for_test(
        old_tools: &[crate::registry::ToolInfo],
        new_tools: &[crate::registry::ToolInfo],
    ) -> (Vec<String>, Vec<String>, Vec<String>, usize) {
        let diff = Self::diff_tool_infos(old_tools, new_tools);
        (
            diff.added_tools,
            diff.removed_tools,
            diff.updated_tools,
            diff.changes_count,
        )
    }
}

struct ToolDiff {
    changes_count: usize,
    added_tools: Vec<String>,
    removed_tools: Vec<String>,
    updated_tools: Vec<String>,
}
