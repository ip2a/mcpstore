use crate::cache::models::{SessionScope, ToolPreferenceState};
use crate::cache::CacheError;
use crate::store::prelude::*;

const TOOL_PREFERENCES_STATE_TYPE: &str = "tool_preferences";

impl MCPStore {
    pub async fn get_tool_preference(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
        tool_name: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        Ok(self
            .get_tool_preferences(agent_id, service_name, tool_name)
            .await?
            .and_then(|state| state.preferences.get(key).cloned()))
    }

    pub async fn set_tool_preference(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
        tool_name: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ToolPreferenceState> {
        Self::validate_tool_preference_key(key)?;
        let target = self
            .resolve_tool_preference_target(agent_id, service_name, tool_name)
            .await?;
        self.update_tool_preferences(&target, |state| {
            state.preferences.insert(key.to_string(), value.clone());
        })
        .await
    }

    pub async fn clear_tool_preference(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
        tool_name: &str,
        key: &str,
    ) -> Result<Option<ToolPreferenceState>> {
        Self::validate_tool_preference_key(key)?;
        let target = self
            .resolve_tool_preference_target(agent_id, service_name, tool_name)
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
        agent_id: Option<&str>,
        service_name: &str,
        tool_name: &str,
    ) -> Result<Option<ToolPreferenceState>> {
        let target = self
            .resolve_tool_preference_target(agent_id, service_name, tool_name)
            .await?;
        self.load_tool_preferences(&target.state_key).await
    }

    async fn resolve_tool_preference_target(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
        tool_name: &str,
    ) -> Result<ToolPreferenceTarget> {
        let context_key = match agent_id {
            Some(agent_id) => {
                Self::build_session_context_key(&SessionScope::Agent, Some(agent_id))?
            }
            None => Self::build_session_context_key(&SessionScope::Store, None)?,
        };
        let (service_global_name, tool_original_name) = if service_name.trim().is_empty() {
            let resolution = self
                .resolve_tool_for_agent(agent_id.unwrap_or(GLOBAL_AGENT_STORE), tool_name)
                .await?;
            (
                resolution.global_service_name,
                resolution.canonical_tool_name,
            )
        } else {
            let service_global_name = match agent_id {
                Some(agent_id) => {
                    self.resolve_service_name_for_agent(agent_id, service_name)
                        .await?
                }
                None => self
                    .find_service(service_name)
                    .await
                    .map(|service| service.name)
                    .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?,
            };
            let tool_original_name = self
                .resolve_tool_original_name_for_preferences(&service_global_name, tool_name)
                .await?;
            (service_global_name, tool_original_name)
        };
        let tool_global_name =
            generate_tool_global_name(&service_global_name, &tool_original_name)?;
        let state_key = Self::tool_preferences_state_key(&context_key, &tool_global_name);
        Ok(ToolPreferenceTarget {
            context_key,
            service_global_name,
            tool_global_name,
            tool_original_name,
            state_key,
        })
    }

    async fn resolve_tool_original_name_for_preferences(
        &self,
        service_global_name: &str,
        tool_name: &str,
    ) -> Result<String> {
        let service = self
            .find_service(service_global_name)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(service_global_name.to_string()))?;
        if service.tools.iter().any(|tool| tool.name == tool_name) {
            return Ok(tool_name.to_string());
        }
        let prefix = format!("{service_global_name}_");
        if let Some(original_name) = tool_name.strip_prefix(&prefix) {
            if service.tools.iter().any(|tool| tool.name == original_name) {
                return Ok(original_name.to_string());
            }
        }
        for tool in service.tools {
            let transformed = self
                .apply_tool_transform(
                    service_global_name,
                    &tool.name,
                    generate_tool_global_name(service_global_name, &tool.name)?,
                    tool.description,
                    tool.input_schema,
                )
                .await?;
            if transformed.display_name == tool_name {
                return Ok(tool.name);
            }
        }
        Err(StoreError::Other(format!(
            "Tool '{tool_name}' not found in service '{service_global_name}'"
        )))
    }

    fn tool_preferences_state_key(context_key: &str, tool_global_name: &str) -> String {
        format!("{context_key}:{tool_global_name}")
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
                serde_json::from_value(value).map_err(|err| {
                    StoreError::Other(format!("Tool preferences deserialization failed: {err}"))
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
                service_global_name: target.service_global_name.clone(),
                tool_global_name: target.tool_global_name.clone(),
                tool_original_name: target.tool_original_name.clone(),
                preferences: serde_json::Map::new(),
                updated_at: now,
                version: 0,
            });
            update(&mut state);
            state.updated_at = now;
            state.version += 1;
            let value =
                serde_json::to_value(&state).map_err(|err| StoreError::Other(err.to_string()))?;
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
    service_global_name: String,
    tool_global_name: String,
    tool_original_name: String,
    state_key: String,
}
