use serde::{Deserialize, Serialize};

use crate::overrides::ComponentOverrideCommon;
use crate::store::prelude::*;

pub(crate) const SERVICE_OVERRIDES_STATE_TYPE: &str = "service_overrides";

/// Gateway-facing projection of a whole service instance: rename it, rewrite
/// its description/icons, or hide it from the aggregated catalog entirely.
/// Absence of a rule means pass-through — the real service name and metadata
/// are exposed as-is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceOverrideRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ServiceOverridePatch {
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
}

impl MCPStore {
    pub async fn set_service_override(
        &self,
        instance_id: InstanceId,
        patch: ServiceOverridePatch,
    ) -> Result<ServiceOverrideRule> {
        self.refresh_from_db_if_needed().await?;
        let instance = self.require_instance(instance_id).await?;
        let loaded = self.get_service_override(instance_id).await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let now = Self::now_timestamp();
        let mut rule = loaded.unwrap_or_else(|| ServiceOverrideRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            common: ComponentOverrideCommon::default(),
            updated_at: now,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.common = patch.common;
        rule.common.display_name = rule
            .common
            .display_name
            .filter(|value| !value.trim().is_empty());
        rule.updated_at = now;
        rule.version += 1;
        self.kernel
            .persistence
            .kv
            .compare_and_put_state(
                SERVICE_OVERRIDES_STATE_TYPE,
                &self.service_override_key(instance_id),
                expected_version,
                serde_json::to_value(&rule).map_err(|err| {
                    Error::new(FailureCode::Internal, err.to_string())
                })?,
            )
            .await?;
        // Best-effort writeback to scopes.toml; openkeyv above is the source of truth.
        if let Err(error) = self
            .writeback_service_override(&rule.service_name, &rule.scope, &rule.common)
            .await
        {
            tracing::warn!("Failed to write back service override to scopes.toml: {error}");
        }
        // Publish after the writeback: the gateway's scopes.toml fingerprint
        // tick reads the file, so the persistent channel is consistent before
        // the (faster) event channel fires.
        self.publish_override_changed(instance_id, &rule.service_name)
            .await;
        Ok(rule)
    }

    pub async fn get_service_override(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<ServiceOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        self.kernel
            .persistence
            .kv
            .get_state(
                SERVICE_OVERRIDES_STATE_TYPE,
                &self.service_override_key(instance_id),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|err| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Service override deserialization failed: {err}"),
                    )
                })
            })
            .transpose()
    }

    pub async fn delete_service_override(&self, instance_id: InstanceId) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        self.kernel
            .persistence
            .kv
            .delete_state(
                SERVICE_OVERRIDES_STATE_TYPE,
                &self.service_override_key(instance_id),
            )
            .await?;
        // Best-effort scopes.toml cleanup so the next load cannot resurrect
        // the deleted rule from its writeback. Skipped when the instance is
        // already gone (nothing to resolve the file path from).
        let mut service_name = None;
        if let Some(instance) = self.kernel.control.registry.find_instance(instance_id).await {
            service_name = Some(instance.service_name.clone());
            if let Err(error) = self
                .writeback_service_override_removal(&instance.service_name, &instance.scope)
                .await
            {
                tracing::warn!(
                    "Failed to clean up service override in scopes.toml: {error}"
                );
            }
        }
        if let Some(service_name) = service_name.as_deref() {
            self.publish_override_changed(instance_id, service_name).await;
        }
        Ok(())
    }

    pub async fn list_service_overrides(&self) -> Result<Vec<ServiceOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        let mut rules = Vec::new();
        for (key, value) in self
            .kernel
            .persistence
            .kv
            .get_all_states_async(SERVICE_OVERRIDES_STATE_TYPE)
            .await?
        {
            let rule: ServiceOverrideRule = serde_json::from_value(value).map_err(|err| {
                Error::new(
                    FailureCode::Internal,
                    format!("Service override deserialization failed for {key}: {err}"),
                )
            })?;
            rules.push(rule);
        }
        rules.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
        Ok(rules)
    }

    /// Catalog-path variant: unreadable rules are logged and skipped so one
    /// corrupt row cannot blank the whole gateway listing. Query path: reads
    /// the shared store directly, no registry hydration.
    pub async fn list_service_overrides_lenient(&self) -> Result<Vec<ServiceOverrideRule>> {
        let mut rules = Vec::new();
        for (key, value) in self
            .kernel
            .persistence
            .kv
            .get_all_states_async(SERVICE_OVERRIDES_STATE_TYPE)
            .await?
        {
            match serde_json::from_value::<ServiceOverrideRule>(value) {
                Ok(rule) => rules.push(rule),
                Err(err) => {
                    tracing::warn!(key = %key, %err, "skipping corrupt service override rule");
                }
            }
        }
        rules.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
        Ok(rules)
    }

    fn service_override_key(&self, instance_id: InstanceId) -> String {
        instance_id.to_string()
    }
}
