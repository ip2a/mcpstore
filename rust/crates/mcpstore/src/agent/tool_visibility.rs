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
        instance_id: InstanceId,
    ) -> Result<Option<ContextToolVisibilityState>> {
        let instance = self.require_instance(instance_id).await?;
        let context_key = Self::build_tool_visibility_context_key(&instance.scope)?;
        self.load_context_tool_visibility(&context_key, instance_id)
            .await
    }

    pub async fn set_context_tool_visibility(
        &self,
        instance_id: InstanceId,
        tool_names: Vec<String>,
    ) -> Result<ContextToolVisibilityState> {
        let instance = self.require_instance(instance_id).await?;
        let context_key = Self::build_tool_visibility_context_key(&instance.scope)?;
        let current_tools = self.list_tool_entries_for_instance(instance_id).await?;
        let mut selected = tool_names
            .into_iter()
            .map(|requested_name| {
                let tool_name = current_tools
                    .iter()
                    .find(|tool| tool.tool_name == requested_name || tool.name == requested_name)
                    .map(|tool| tool.tool_name.clone())
                    .unwrap_or(requested_name);
                SessionToolItem {
                    instance_id,
                    service_name: instance.service_name.clone(),
                    scope: instance.scope.clone(),
                    tool_name,
                }
            })
            .collect::<Vec<_>>();
        selected.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
        selected.dedup_by(|left, right| left.tool_name == right.tool_name);
        self.update_context_tool_visibility(&context_key, instance_id, |state| {
            state.tools = selected.clone();
        })
        .await
    }

    pub async fn clear_context_tool_visibility(&self, instance_id: InstanceId) -> Result<()> {
        let instance = self.require_instance(instance_id).await?;
        let context_key = Self::build_tool_visibility_context_key(&instance.scope)?;
        let state_key = Self::context_tool_visibility_state_key(&context_key, instance_id);
        self.cache
            .delete_state(CONTEXT_TOOL_VISIBILITY_STATE_TYPE, &state_key)
            .await
            .map_err(Into::into)
    }

    pub async fn list_tool_entries_for_instance_with_filter(
        &self,
        instance_id: InstanceId,
        filter: ToolVisibilityFilter,
    ) -> Result<Vec<ScopedToolEntry>> {
        let policy = EffectiveToolPolicy::resolve(self, instance_id).await?;
        Ok(match filter {
            ToolVisibilityFilter::All => policy.all,
            ToolVisibilityFilter::Available => policy.available,
            ToolVisibilityFilter::Removed => policy.removed,
        })
    }

    pub(crate) async fn ensure_context_tool_allowed(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<()> {
        let policy = EffectiveToolPolicy::resolve(self, instance_id).await?;
        if policy
            .available
            .iter()
            .any(|tool| tool.tool_name == tool_name)
        {
            Ok(())
        } else {
            Err(StoreError::ToolNotAvailable {
                instance_id,
                tool_name: tool_name.to_string(),
            })
        }
    }

    fn build_tool_visibility_context_key(scope: &ScopeRef) -> Result<String> {
        match scope {
            ScopeRef::Store => Self::build_session_context_key(&SessionScope::Store, None),
            ScopeRef::Agent { agent_id } => {
                Self::build_session_context_key(&SessionScope::Agent, Some(agent_id))
            }
        }
    }

    fn context_tool_visibility_state_key(context_key: &str, instance_id: InstanceId) -> String {
        format!("{context_key}:{instance_id}")
    }

    async fn load_context_tool_visibility(
        &self,
        context_key: &str,
        instance_id: InstanceId,
    ) -> Result<Option<ContextToolVisibilityState>> {
        let state_key = Self::context_tool_visibility_state_key(context_key, instance_id);
        match self
            .cache
            .get_state(CONTEXT_TOOL_VISIBILITY_STATE_TYPE, &state_key)
            .await?
        {
            Some(value) => serde_json::from_value(value)
                .map(Some)
                .map_err(|error| StoreError::Other(error.to_string())),
            None => Ok(None),
        }
    }

    async fn update_context_tool_visibility<F>(
        &self,
        context_key: &str,
        instance_id: InstanceId,
        mut update: F,
    ) -> Result<ContextToolVisibilityState>
    where
        F: FnMut(&mut ContextToolVisibilityState),
    {
        let state_key = Self::context_tool_visibility_state_key(context_key, instance_id);
        let instance = self.require_instance(instance_id).await?;
        for _ in 0..3 {
            let now = Self::now_timestamp();
            let current = self
                .load_context_tool_visibility(context_key, instance_id)
                .await?;
            let expected_version = current.as_ref().map(|state| state.version);
            let mut state = current.unwrap_or_else(|| ContextToolVisibilityState {
                context_key: context_key.to_string(),
                instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
                mode: ToolVisibilityMode::Allowlist,
                tools: Vec::new(),
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

#[derive(Debug, Clone)]
pub struct EffectiveToolPolicy {
    pub all: Vec<ScopedToolEntry>,
    pub available: Vec<ScopedToolEntry>,
    pub removed: Vec<ScopedToolEntry>,
    pub stale: Vec<String>,
}

impl EffectiveToolPolicy {
    pub async fn resolve(store: &MCPStore, instance_id: InstanceId) -> Result<Self> {
        let instance = store.require_instance(instance_id).await?;
        let all = store.list_tool_entries_for_instance(instance_id).await?;
        let visibility = store
            .load_context_tool_visibility(
                &MCPStore::build_tool_visibility_context_key(&instance.scope)?,
                instance_id,
            )
            .await?;
        let Some(visibility) = visibility else {
            return Ok(Self {
                available: all.clone(),
                all,
                removed: Vec::new(),
                stale: Vec::new(),
            });
        };
        let allowlist = visibility
            .tools
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<HashSet<_>>();
        let known = all
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<HashSet<_>>();
        let mut available = Vec::new();
        let mut removed = Vec::new();
        for tool in &all {
            if allowlist.contains(tool.tool_name.as_str()) {
                available.push(tool.clone());
            } else {
                removed.push(tool.clone());
            }
        }
        let mut stale = visibility
            .tools
            .iter()
            .filter(|tool| !known.contains(tool.tool_name.as_str()))
            .map(|tool| tool.tool_name.clone())
            .collect::<Vec<_>>();
        stale.sort();
        stale.dedup();
        Ok(Self {
            all,
            available,
            removed,
            stale,
        })
    }
}
