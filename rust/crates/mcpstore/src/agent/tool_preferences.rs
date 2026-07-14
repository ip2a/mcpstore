use crate::cache::models::{SessionScope, ToolPreferenceState};
use crate::cache::CacheError;
use crate::store::prelude::*;

const TOOL_PREFERENCES_STATE_TYPE: &str = "tool_preferences";

impl MCPStore {
    pub async fn get_tool_preference(
        &self,
        scope: &ScopeRef,
        instance_id: InstanceId,
        tool_name: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        Ok(self
            .get_tool_preferences(scope, instance_id, tool_name)
            .await?
            .and_then(|state| state.preferences.get(key).cloned()))
    }

    pub async fn set_tool_preference(
        &self,
        scope: &ScopeRef,
        instance_id: InstanceId,
        tool_name: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ToolPreferenceState> {
        Self::validate_tool_preference_key(key)?;
        let target = self
            .tool_preference_target(scope, instance_id, tool_name)
            .await?;
        self.update_tool_preferences(&target, |state| {
            state.preferences.insert(key.to_string(), value.clone());
        })
        .await
    }

    pub async fn clear_tool_preference(
        &self,
        scope: &ScopeRef,
        instance_id: InstanceId,
        tool_name: &str,
        key: &str,
    ) -> Result<Option<ToolPreferenceState>> {
        Self::validate_tool_preference_key(key)?;
        let target = self
            .tool_preference_target(scope, instance_id, tool_name)
            .await?;
        let Some(current) = self.load_tool_preferences(&target.state_key).await? else {
            return Ok(None);
        };
        if !current.preferences.contains_key(key) {
            return Ok(Some(current));
        }
        let state = self
            .update_tool_preferences(&target, |state| {
                state.preferences.remove(key);
            })
            .await?;
        if state.preferences.is_empty() {
            self.cache
                .delete_state(TOOL_PREFERENCES_STATE_TYPE, &target.state_key)
                .await?;
            return Ok(None);
        }
        Ok(Some(state))
    }

    pub async fn get_tool_preferences(
        &self,
        scope: &ScopeRef,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolPreferenceState>> {
        let target = self
            .tool_preference_target(scope, instance_id, tool_name)
            .await?;
        self.load_tool_preferences(&target.state_key).await
    }

    async fn tool_preference_target(
        &self,
        scope: &ScopeRef,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<ToolPreferenceTarget> {
        let instance = self.require_instance_in_scope(instance_id, scope).await?;
        if self
            .registry
            .find_tool(instance_id, tool_name)
            .await
            .is_none()
        {
            return Err(StoreError::Other(format!(
                "Tool '{tool_name}' not found in service instance '{instance_id}'"
            )));
        }
        let context_key = match scope {
            ScopeRef::Store => Self::build_session_context_key(&SessionScope::Store, None)?,
            ScopeRef::Agent { agent_id } => {
                Self::build_session_context_key(&SessionScope::Agent, Some(agent_id))?
            }
        };
        let state_key = Self::tool_preferences_state_key(&context_key, instance_id, tool_name);
        Ok(ToolPreferenceTarget {
            context_key,
            instance_id,
            service_name: instance.service_name,
            scope: instance.scope,
            tool_name: tool_name.to_string(),
            state_key,
        })
    }

    fn tool_preferences_state_key(
        context_key: &str,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> String {
        format!("{context_key}:{instance_id}:{tool_name}")
    }

    fn validate_tool_preference_key(key: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(StoreError::Other(
                "Tool preference key cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    async fn load_tool_preferences(&self, state_key: &str) -> Result<Option<ToolPreferenceState>> {
        self.cache
            .get_state(TOOL_PREFERENCES_STATE_TYPE, state_key)
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|error| {
                    StoreError::Other(format!("Tool preferences deserialization failed: {error}"))
                })
            })
            .transpose()
    }

    async fn update_tool_preferences<F>(
        &self,
        target: &ToolPreferenceTarget,
        mut update: F,
    ) -> Result<ToolPreferenceState>
    where
        F: FnMut(&mut ToolPreferenceState),
    {
        for _ in 0..3 {
            let now = Self::now_timestamp();
            let current = self.load_tool_preferences(&target.state_key).await?;
            let expected_version = current.as_ref().map(|state| state.version);
            let mut state = current.unwrap_or_else(|| ToolPreferenceState {
                context_key: target.context_key.clone(),
                instance_id: target.instance_id,
                service_name: target.service_name.clone(),
                scope: target.scope.clone(),
                tool_name: target.tool_name.clone(),
                preferences: serde_json::Map::new(),
                updated_at: now,
                version: 0,
            });
            update(&mut state);
            state.updated_at = now;
            state.version += 1;
            let value = serde_json::to_value(&state)
                .map_err(|error| StoreError::Other(error.to_string()))?;
            match self
                .cache
                .compare_and_put_state(
                    TOOL_PREFERENCES_STATE_TYPE,
                    &target.state_key,
                    expected_version,
                    value,
                )
                .await
            {
                Ok(()) => return Ok(state),
                Err(CacheError::Conflict(_)) => continue,
                Err(error) => return Err(StoreError::Cache(error)),
            }
        }
        Err(StoreError::Cache(CacheError::Conflict(format!(
            "tool preferences conflict after retries: state_key={}",
            target.state_key
        ))))
    }
}

struct ToolPreferenceTarget {
    context_key: String,
    instance_id: InstanceId,
    service_name: String,
    scope: ScopeRef,
    tool_name: String,
    state_key: String,
}
