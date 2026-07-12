use std::collections::{BTreeMap, BTreeSet};

use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_changed_tools_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
        force_refresh: bool,
    ) -> Result<ToolChangeSummary> {
        self.refresh_from_db_if_needed().await?;
        let services = self
            .tool_change_service_names(agent_id, service_name)
            .await?;

        if self.source_mode == SourceMode::Db {
            for service_name in &services {
                self.queue_service_refresh_tools_request(service_name, force_refresh)
                    .await?;
            }
            let timestamp = chrono::Utc::now().timestamp();
            let total_services = services.len();
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
                    "queued_services": services,
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

        let mut results = Vec::with_capacity(services.len());
        let mut errors = Vec::new();

        for global_service_name in services {
            match self
                .refresh_service_tools_with_diff(&global_service_name, force_refresh)
                .await
            {
                Ok(result) => results.push(result),
                Err(error) => errors.push(serde_json::json!({
                    "service_name": global_service_name,
                    "error": error.to_string()
                })),
            }
        }

        let changed_services = results
            .iter()
            .filter(|result| result.changed)
            .map(|result| result.service_name.clone())
            .collect::<Vec<_>>();
        let total_changes = results
            .iter()
            .map(|result| result.changes_count)
            .sum::<usize>();
        let timestamp = chrono::Utc::now().timestamp();

        Ok(ToolChangeSummary {
            changed: !changed_services.is_empty(),
            services: changed_services,
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
        service_name: &str,
        force_refresh: bool,
    ) -> Result<ToolChangeServiceResult> {
        self.refresh_from_db_if_needed().await?;
        let Some(mut entry) = self.registry.find_service(service_name).await else {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        };

        if !force_refresh && !self.pool.is_connected(service_name).await {
            self.ensure_service_connected(service_name).await?;
        } else if force_refresh {
            self.pool.disconnect(service_name).await.ok();
            self.connect_service_internal(service_name, false).await?;
        }

        let old_tools = entry.tools.clone();
        let new_tools = self.pool.list_tools(service_name).await?;
        let tool_infos = new_tools.into_iter().map(Into::into).collect::<Vec<_>>();

        let diff = Self::diff_tool_infos(&old_tools, &tool_infos);
        entry.tools = tool_infos;
        entry.status = ConnectionStatus::Connected;
        self.registry.register(entry).await;

        let tools = self.registry.list_service_tools(service_name).await;
        self.cache_service_connected(service_name, &tools).await?;

        let timestamp = chrono::Utc::now().timestamp();
        self.cache
            .put_event(
                "service",
                &format!("{service_name}:tools_refreshed:{timestamp}"),
                serde_json::json!({
                    "event": "service_tools_refreshed",
                    "service": service_name,
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
            service_name: service_name.to_string(),
            client_id: service_name.to_string(),
            timestamp,
        })
    }

    async fn tool_change_service_names(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<String>> {
        match (agent_id, service_name) {
            (Some(agent_id), Some(service_name)) => Ok(vec![
                self.resolve_service_name_for_agent(agent_id, service_name)
                    .await?,
            ]),
            (Some(agent_id), None) => self.list_agent_service_names(agent_id).await,
            (None, Some(service_name)) => Ok(vec![service_name.to_string()]),
            (None, None) => Ok(self
                .registry
                .list_services()
                .await
                .into_iter()
                .map(|service| service.name)
                .collect()),
        }
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
