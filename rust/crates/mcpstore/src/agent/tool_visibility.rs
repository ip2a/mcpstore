use std::collections::HashSet;

use crate::cache::models::{
    ContextToolVisibilityState, SessionScope, SessionToolItem, ToolVisibilityMode,
};
use crate::cache::CacheError;
use crate::store::prelude::*;

const CONTEXT_TOOL_VISIBILITY_STATE_TYPE: &str = "context_tool_visibility";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolVisibilityFilter {
    All,
    Available,
    Removed,
}

impl MCPStore {
    pub async fn get_context_tool_visibility(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<Option<ContextToolVisibilityState>> {
        let context_key = Self::build_tool_visibility_context_key(agent_id)?;
        let global_service_name = self
            .resolve_context_tool_visibility_service_name(agent_id, service_name)
            .await?;
        self.load_context_tool_visibility(&context_key, &global_service_name)
            .await
    }

    pub async fn set_context_tool_visibility(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
        tool_names: Vec<String>,
    ) -> Result<ContextToolVisibilityState> {
        let context_key = Self::build_tool_visibility_context_key(agent_id)?;
        let global_service_name = self
            .resolve_context_tool_visibility_service_name(agent_id, service_name)
            .await?;
        let all_tools = self
            .list_tool_entries_scoped(agent_id, Some(service_name))
            .await?;
        let allowlist: HashSet<String> = tool_names.into_iter().collect();
        let mut selected = Vec::new();
        for tool in all_tools {
            if Self::tool_entry_matches_any_name(&tool, &allowlist) {
                selected.push(SessionToolItem {
                    service_global_name: tool.global_service_name,
                    tool_global_name: tool.global_tool_name,
                    tool_original_name: tool.original_name,
                });
            }
        }
        selected.sort_by(|left, right| left.tool_global_name.cmp(&right.tool_global_name));
        selected.dedup_by(|left, right| left.tool_global_name == right.tool_global_name);
        self.update_context_tool_visibility(&context_key, &global_service_name, |state| {
            state.tools = selected.clone();
        })
        .await
    }

    pub async fn clear_context_tool_visibility(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<()> {
        let context_key = Self::build_tool_visibility_context_key(agent_id)?;
        let global_service_name = self
            .resolve_context_tool_visibility_service_name(agent_id, service_name)
            .await?;
        let state_key = Self::context_tool_visibility_state_key(&context_key, &global_service_name);
        self.cache
            .delete_state(CONTEXT_TOOL_VISIBILITY_STATE_TYPE, &state_key)
            .await
            .map_err(Into::into)
    }

    pub async fn list_tool_entries_scoped_with_filter(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
        filter: ToolVisibilityFilter,
    ) -> Result<Vec<ScopedToolEntry>> {
        let tools = self
            .list_tool_entries_scoped(agent_id, service_name)
            .await?;
        self.apply_context_tool_visibility(agent_id, service_name, tools, filter)
            .await
    }

    async fn apply_context_tool_visibility(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
        tools: Vec<ScopedToolEntry>,
        filter: ToolVisibilityFilter,
    ) -> Result<Vec<ScopedToolEntry>> {
        if filter == ToolVisibilityFilter::All {
            return Ok(tools);
        }
        let context_key = Self::build_tool_visibility_context_key(agent_id)?;
        let mut selected = Vec::new();
        for tool in tools {
            let visibility = self
                .load_context_tool_visibility(&context_key, &tool.global_service_name)
                .await?;
            let is_visible = visibility.as_ref().is_none_or(|state| {
                state.tools.iter().any(|item| {
                    item.tool_global_name == tool.global_tool_name
                        || item.tool_original_name == tool.original_name
                        || item.tool_original_name == tool.name
                })
            });
            if (filter == ToolVisibilityFilter::Available && is_visible)
                || (filter == ToolVisibilityFilter::Removed && !is_visible)
            {
                selected.push(tool);
            }
        }
        if service_name.is_some() {
            selected.sort_by(|left, right| left.name.cmp(&right.name));
        }
        Ok(selected)
    }

    async fn resolve_context_tool_visibility_service_name(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<String> {
        match agent_id {
            Some(agent_id) => {
                self.resolve_service_name_for_agent(agent_id, service_name)
                    .await
            }
            None => self
                .find_service(service_name)
                .await
                .map(|service| service.name)
                .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string())),
        }
    }

    fn build_tool_visibility_context_key(agent_id: Option<&str>) -> Result<String> {
        match agent_id {
            Some(agent_id) => Self::build_session_context_key(&SessionScope::Agent, Some(agent_id)),
            None => Self::build_session_context_key(&SessionScope::Store, None),
        }
    }

    fn context_tool_visibility_state_key(context_key: &str, service_global_name: &str) -> String {
        format!("{context_key}:{service_global_name}")
    }

    fn tool_entry_matches_any_name(tool: &ScopedToolEntry, names: &HashSet<String>) -> bool {
        names.contains(&tool.name)
            || names.contains(&tool.original_name)
            || names.contains(&tool.global_tool_name)
    }

    async fn load_context_tool_visibility(
        &self,
        context_key: &str,
        service_global_name: &str,
    ) -> Result<Option<ContextToolVisibilityState>> {
        let state_key = Self::context_tool_visibility_state_key(context_key, service_global_name);
        match self
            .cache
            .get_state(CONTEXT_TOOL_VISIBILITY_STATE_TYPE, &state_key)
            .await?
        {
            Some(value) => serde_json::from_value(value)
                .map(Some)
                .map_err(|e| StoreError::Other(e.to_string())),
            None => Ok(None),
        }
    }

    async fn update_context_tool_visibility<F>(
        &self,
        context_key: &str,
        service_global_name: &str,
        mut update: F,
    ) -> Result<ContextToolVisibilityState>
    where
        F: FnMut(&mut ContextToolVisibilityState),
    {
        let state_key = Self::context_tool_visibility_state_key(context_key, service_global_name);
        for _ in 0..3 {
            let now = Self::now_timestamp();
            let current = self
                .load_context_tool_visibility(context_key, service_global_name)
                .await?;
            let expected_version = current.as_ref().map(|state| state.version);
            let mut state = current.unwrap_or_else(|| ContextToolVisibilityState {
                context_key: context_key.to_string(),
                service_global_name: service_global_name.to_string(),
                mode: ToolVisibilityMode::Allowlist,
                tools: Vec::new(),
                updated_at: now,
                version: 0,
            });
            update(&mut state);
            state.updated_at = now;
            state.version += 1;
            let value =
                serde_json::to_value(&state).map_err(|e| StoreError::Other(e.to_string()))?;
            match self
                .cache
                .compare_and_put_state(
                    CONTEXT_TOOL_VISIBILITY_STATE_TYPE,
                    &state_key,
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
            "context tool visibility conflict after retries: state_key={state_key}"
        ))))
    }
}
