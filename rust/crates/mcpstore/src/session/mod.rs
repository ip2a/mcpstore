use std::{
    collections::{HashMap, HashSet},
    future::Future,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cache::models::{
    SessionContextState, SessionEntity, SessionEvent, SessionEventType, SessionScope,
    SessionServiceItem, SessionServiceRelation, SessionStateData, SessionStatus,
    SessionStatusState, SessionToolItem, SessionToolVisibility, ToolVisibilityMode,
};
use crate::cache::CacheError;
use crate::store::prelude::*;

const DEFAULT_SESSION_RETRY_ATTEMPTS: usize = 3;

const SESSION_ENTITY_TYPE: &str = "sessions";
const SESSION_SERVICES_RELATION_TYPE: &str = "session_services";
const SESSION_TOOLS_RELATION_TYPE: &str = "session_tools";
const SESSION_STATUS_STATE_TYPE: &str = "session_status";
const SESSION_STATE_TYPE: &str = "session_state";
const SESSION_CONTEXT_STATE_TYPE: &str = "session_context";
const SESSION_EVENT_TYPE: &str = "session_events";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateSessionRequest {
    pub session_id: String,
    pub scope: SessionScope,
    pub agent_id: Option<String>,
    pub lease_seconds: Option<i64>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionImportReport {
    pub sessions_imported: usize,
    pub session_service_relations_imported: usize,
    pub session_tool_relations_imported: usize,
    pub session_status_states_imported: usize,
    pub session_state_records_imported: usize,
    pub session_context_states_imported: usize,
    pub session_events_imported: usize,
    pub sessions_unchanged: usize,
    pub session_service_relations_unchanged: usize,
    pub session_tool_relations_unchanged: usize,
    pub session_status_states_unchanged: usize,
    pub session_state_records_unchanged: usize,
    pub session_context_states_unchanged: usize,
    pub session_events_unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionCleanupReport {
    pub refreshed_sessions: usize,
    pub expired_sessions: usize,
    pub cleared_active_session: bool,
    pub cleared_auto_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRestartReport {
    pub restarted_instances: Vec<InstanceId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRetryPolicy {
    pub max_attempts: usize,
    pub delay_millis: u64,
}

impl Default for SessionRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_SESSION_RETRY_ATTEMPTS,
            delay_millis: 0,
        }
    }
}

impl SessionRetryPolicy {
    pub fn new(max_attempts: usize) -> Self {
        Self {
            max_attempts,
            ..Self::default()
        }
    }

    pub fn delay_millis(mut self, delay_millis: u64) -> Self {
        self.delay_millis = delay_millis;
        self
    }
}

struct ValidatedSessionSnapshot {
    entities: Vec<(String, SessionEntity, serde_json::Value)>,
    service_relations: Vec<(String, SessionServiceRelation, serde_json::Value)>,
    tool_relations: Vec<(String, SessionToolVisibility, serde_json::Value)>,
    status_states: Vec<(String, SessionStatusState, serde_json::Value)>,
    session_states: Vec<(String, SessionStateData, serde_json::Value)>,
    context_states: Vec<(String, SessionContextState, serde_json::Value)>,
    events: Vec<(String, SessionEvent, serde_json::Value)>,
}

impl CreateSessionRequest {
    pub fn store(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            scope: SessionScope::Store,
            agent_id: None,
            lease_seconds: None,
            metadata: serde_json::json!({}),
        }
    }

    pub fn agent(session_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            scope: SessionScope::Agent,
            agent_id: Some(agent_id.into()),
            lease_seconds: None,
            metadata: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionToolSelection {
    pub instance_id: InstanceId,
    pub tool_name: String,
}

pub struct SessionBuilder<'a> {
    store: &'a MCPStore,
    request: CreateSessionRequest,
}

impl<'a> SessionBuilder<'a> {
    fn new(store: &'a MCPStore, session_id: impl Into<String>) -> Self {
        Self {
            store,
            request: CreateSessionRequest::store(session_id),
        }
    }

    pub fn for_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.request.scope = SessionScope::Agent;
        self.request.agent_id = Some(agent_id.into());
        self
    }

    pub fn for_store(mut self) -> Self {
        self.request.scope = SessionScope::Store;
        self.request.agent_id = None;
        self
    }

    pub fn lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.request.lease_seconds = Some(lease_seconds);
        self
    }

    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.request.metadata = metadata;
        self
    }

    pub async fn create(self) -> Result<SessionContext<'a>> {
        let entity = self.store.create_session(self.request).await?;
        Ok(SessionContext::new(self.store, entity))
    }

    pub async fn create_or_get(self) -> Result<SessionContext<'a>> {
        if let Some(entity) = self
            .store
            .find_session(
                self.request.scope.clone(),
                self.request.agent_id.as_deref(),
                &self.request.session_id,
            )
            .await?
        {
            return Ok(SessionContext::new(self.store, entity));
        }
        self.create().await
    }

    pub async fn get(self) -> Result<Option<SessionContext<'a>>> {
        Ok(self
            .store
            .find_session(
                self.request.scope,
                self.request.agent_id.as_deref(),
                &self.request.session_id,
            )
            .await?
            .map(|entity| SessionContext::new(self.store, entity)))
    }
}

#[derive(Clone)]
pub struct SessionContext<'a> {
    store: &'a MCPStore,
    session_key: String,
}

impl<'a> SessionContext<'a> {
    fn new(store: &'a MCPStore, entity: SessionEntity) -> Self {
        Self {
            store,
            session_key: entity.session_key,
        }
    }

    pub fn from_key(store: &'a MCPStore, session_key: impl Into<String>) -> Self {
        Self {
            store,
            session_key: session_key.into(),
        }
    }

    pub fn session_key(&self) -> &str {
        &self.session_key
    }

    pub async fn entity(&self) -> Result<SessionEntity> {
        self.store.require_session(&self.session_key).await
    }

    pub async fn status(&self) -> Result<SessionStatusState> {
        self.store
            .get_session_status(&self.session_key)
            .await?
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "Session not found: session_key={}",
                    self.session_key
                ))
            })
    }

    pub async fn extend(&self, lease_seconds: i64) -> Result<SessionEntity> {
        self.store
            .extend_session(&self.session_key, lease_seconds)
            .await
    }

    pub async fn extend_with_retry(
        &self,
        lease_seconds: i64,
        policy: SessionRetryPolicy,
    ) -> Result<SessionEntity> {
        self.store
            .extend_session_with_retry(&self.session_key, lease_seconds, policy)
            .await
    }

    pub async fn close(&self) -> Result<SessionStatusState> {
        self.store.close_session(&self.session_key, None).await
    }

    pub async fn close_with_reason(&self, reason: impl Into<String>) -> Result<SessionStatusState> {
        self.store
            .close_session(&self.session_key, Some(reason.into()))
            .await
    }

    pub async fn bind_service(&self, instance_id: InstanceId) -> Result<SessionServiceRelation> {
        self.store
            .bind_service_to_session(&self.session_key, instance_id)
            .await
    }

    pub async fn bind_service_with_retry(
        &self,
        instance_id: InstanceId,
        policy: SessionRetryPolicy,
    ) -> Result<SessionServiceRelation> {
        self.store
            .bind_service_to_session_with_retry(&self.session_key, instance_id, policy)
            .await
    }

    pub async fn unbind_service(&self, instance_id: InstanceId) -> Result<SessionServiceRelation> {
        self.store
            .unbind_service_from_session(&self.session_key, instance_id)
            .await
    }

    pub async fn unbind_service_with_retry(
        &self,
        instance_id: InstanceId,
        policy: SessionRetryPolicy,
    ) -> Result<SessionServiceRelation> {
        self.store
            .unbind_service_from_session_with_retry(&self.session_key, instance_id, policy)
            .await
    }

    pub async fn list_services(&self) -> Result<Vec<SessionServiceItem>> {
        self.store.list_session_services(&self.session_key).await
    }

    pub async fn set_tool_visibility(
        &self,
        selections: Vec<SessionToolSelection>,
    ) -> Result<SessionToolVisibility> {
        self.store
            .set_session_tool_visibility(&self.session_key, selections)
            .await
    }

    pub async fn list_session_tools(&self) -> Result<Vec<SessionToolItem>> {
        self.store.list_session_tools(&self.session_key).await
    }

    pub async fn set_state(&self, key: &str, value: serde_json::Value) -> Result<SessionStateData> {
        self.store
            .set_session_state(&self.session_key, key, value)
            .await
    }

    pub async fn set_state_with_retry(
        &self,
        key: &str,
        value: serde_json::Value,
        policy: SessionRetryPolicy,
    ) -> Result<SessionStateData> {
        self.store
            .set_session_state_with_retry(&self.session_key, key, value, policy)
            .await
    }

    pub async fn get_state(&self, key: &str) -> Result<Option<serde_json::Value>> {
        self.store
            .get_session_state_value(&self.session_key, key)
            .await
    }

    pub async fn list_state(&self) -> Result<SessionStateData> {
        self.store.list_session_state(&self.session_key).await
    }

    pub async fn delete_state(&self, key: &str) -> Result<SessionStateData> {
        self.store
            .delete_session_state(&self.session_key, key)
            .await
    }

    pub async fn delete_state_with_retry(
        &self,
        key: &str,
        policy: SessionRetryPolicy,
    ) -> Result<SessionStateData> {
        self.store
            .delete_session_state_with_retry(&self.session_key, key, policy)
            .await
    }

    pub async fn clear_state(&self) -> Result<SessionStateData> {
        self.store.clear_session_state(&self.session_key).await
    }

    pub async fn list_tools(&self) -> Result<Vec<ScopedToolEntry>> {
        self.store.list_tools_in_session(&self.session_key).await
    }

    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.store
            .call_tool_in_session(&self.session_key, instance_id, tool_name, args)
            .await
    }

    pub async fn list_resources(&self) -> Result<Vec<serde_json::Value>> {
        self.store
            .list_resources_in_session(&self.session_key)
            .await
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<serde_json::Value>> {
        self.store
            .list_resource_templates_in_session(&self.session_key)
            .await
    }

    pub async fn read_resource(
        &self,
        uri: &str,
        instance_id: InstanceId,
    ) -> Result<serde_json::Value> {
        self.store
            .read_resource_in_session(&self.session_key, uri, instance_id)
            .await
    }

    pub async fn list_prompts(&self) -> Result<Vec<serde_json::Value>> {
        self.store.list_prompts_in_session(&self.session_key).await
    }

    pub async fn get_prompt(
        &self,
        prompt_name: &str,
        arguments: serde_json::Value,
        instance_id: InstanceId,
    ) -> Result<serde_json::Value> {
        self.store
            .get_prompt_in_session(&self.session_key, prompt_name, arguments, instance_id)
            .await
    }
}

impl MCPStore {
    pub fn session(&self, session_id: impl Into<String>) -> SessionBuilder<'_> {
        SessionBuilder::new(self, session_id)
    }

    pub fn session_by_key(&self, session_key: impl Into<String>) -> SessionContext<'_> {
        SessionContext::from_key(self, session_key)
    }

    pub fn build_session_key(
        scope: &SessionScope,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<String> {
        Self::validate_session_id(session_id)?;
        match scope {
            SessionScope::Store => {
                if agent_id.is_some() {
                    return Err(StoreError::Other(
                        "store-scoped sessions must not include agent_id".to_string(),
                    ));
                }
                Ok(format!("store:{session_id}"))
            }
            SessionScope::Agent => {
                let agent_id = agent_id.ok_or_else(|| {
                    StoreError::Other("agent-scoped sessions require agent_id".to_string())
                })?;
                Self::validate_session_id(agent_id)?;
                Ok(format!("agent:{agent_id}:{session_id}"))
            }
        }
    }

    pub async fn create_session(&self, request: CreateSessionRequest) -> Result<SessionEntity> {
        Self::validate_lease_seconds(request.lease_seconds)?;
        let session_key = Self::build_session_key(
            &request.scope,
            request.agent_id.as_deref(),
            &request.session_id,
        )?;
        if self.get_session(&session_key).await?.is_some() {
            return Err(StoreError::Other(format!(
                "Session already exists: session_key={session_key}"
            )));
        }

        let now = Self::now_timestamp();
        let expires_at = request.lease_seconds.map(|seconds| now + seconds);
        let session = SessionEntity {
            session_key: session_key.clone(),
            session_id: request.session_id,
            scope: request.scope,
            agent_id: request.agent_id,
            created_at: now,
            updated_at: now,
            last_active: now,
            lease_seconds: request.lease_seconds,
            expires_at,
            version: 1,
            metadata: request.metadata,
        };
        let status = SessionStatusState {
            session_key: session_key.clone(),
            status: SessionStatus::Active,
            updated_at: now,
            version: 1,
            reason: None,
        };

        self.store_session_entity(&session, None).await?;
        self.store_session_status(&status, None).await?;
        self.append_session_event(
            &session_key,
            SessionEventType::Create,
            serde_json::json!({}),
        )
        .await?;
        Ok(session)
    }

    pub async fn get_session(&self, session_key: &str) -> Result<Option<SessionEntity>> {
        let Some(value) = self
            .cache
            .get_entity(SESSION_ENTITY_TYPE, session_key)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Self::decode_session_entity(value)?))
    }

    pub async fn get_session_status(
        &self,
        session_key: &str,
    ) -> Result<Option<SessionStatusState>> {
        let session = self.require_session(session_key).await?;
        let now = Self::now_timestamp();
        let mut status = self
            .load_session_status(session_key)
            .await?
            .unwrap_or_else(|| Self::default_session_status(session_key, now));
        if status.status == SessionStatus::Active {
            if let Some(expires_at) = session.expires_at {
                if expires_at <= now {
                    let expected_version = Some(status.version);
                    status.status = SessionStatus::Expired;
                    status.updated_at = now;
                    status.version += 1;
                    status.reason = Some("lease expired".to_string());
                    self.store_session_status(&status, expected_version).await?;
                    self.append_session_event(
                        session_key,
                        SessionEventType::Expire,
                        serde_json::json!({ "expires_at": expires_at }),
                    )
                    .await?;
                }
            }
        }
        Ok(Some(status))
    }

    pub async fn find_session(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<Option<SessionEntity>> {
        let session_key = Self::build_session_key(&scope, agent_id, session_id)?;
        self.get_session(&session_key).await
    }

    pub fn build_session_context_key(
        scope: &SessionScope,
        agent_id: Option<&str>,
    ) -> Result<String> {
        match scope {
            SessionScope::Store => {
                if agent_id.is_some() {
                    return Err(StoreError::Other(
                        "store-scoped session context must not include agent_id".to_string(),
                    ));
                }
                Ok("store".to_string())
            }
            SessionScope::Agent => {
                let agent_id = agent_id.ok_or_else(|| {
                    StoreError::Other("agent-scoped session context requires agent_id".to_string())
                })?;
                Self::validate_session_id(agent_id)?;
                Ok(format!("agent:{agent_id}"))
            }
        }
    }

    pub async fn get_session_context_state(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
    ) -> Result<Option<SessionContextState>> {
        let context_key = Self::build_session_context_key(&scope, agent_id)?;
        self.load_session_context_state(&context_key).await
    }

    pub async fn set_active_session_for_context(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
        session_key: Option<&str>,
    ) -> Result<SessionContextState> {
        if let Some(session_key) = session_key {
            self.ensure_session_matches_context(&scope, agent_id, session_key)
                .await?;
        }
        self.update_session_context_state(scope, agent_id, |state| {
            state.active_session_key = session_key.map(ToOwned::to_owned);
        })
        .await
    }

    pub async fn get_active_session_for_context(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
    ) -> Result<Option<SessionEntity>> {
        let Some(state) = self.get_session_context_state(scope, agent_id).await? else {
            return Ok(None);
        };
        if let Some(session_key) = state.active_session_key {
            if let Some(session) = self.get_active_session_entity(&session_key).await? {
                return Ok(Some(session));
            }
        }
        if let Some(session_key) = state.auto_session_key {
            return self.get_active_session_entity(&session_key).await;
        }
        Ok(None)
    }

    pub async fn enable_auto_session_for_context(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
        session_key: &str,
    ) -> Result<SessionContextState> {
        self.ensure_session_matches_context(&scope, agent_id, session_key)
            .await?;
        self.update_session_context_state(scope, agent_id, |state| {
            state.auto_session_key = Some(session_key.to_string());
        })
        .await
    }

    pub async fn disable_auto_session_for_context(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
    ) -> Result<SessionContextState> {
        self.update_session_context_state(scope, agent_id, |state| {
            state.auto_session_key = None;
        })
        .await
    }

    pub async fn is_auto_session_enabled_for_context(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
    ) -> Result<bool> {
        Ok(self
            .get_session_context_state(scope, agent_id)
            .await?
            .and_then(|state| state.auto_session_key)
            .is_some())
    }

    pub async fn list_sessions(
        &self,
        scope: Option<SessionScope>,
        agent_id: Option<&str>,
    ) -> Result<Vec<SessionEntity>> {
        let entries = self
            .cache
            .get_all_entities_async(SESSION_ENTITY_TYPE)
            .await?;
        let mut sessions = Vec::with_capacity(entries.len());
        for value in entries.into_values() {
            let session = Self::decode_session_entity(value)?;
            if let Some(expected_scope) = &scope {
                if &session.scope != expected_scope {
                    continue;
                }
            }
            if let Some(expected_agent_id) = agent_id {
                if session.agent_id.as_deref() != Some(expected_agent_id) {
                    continue;
                }
            }
            sessions.push(session);
        }
        sessions.sort_by(|left, right| left.session_key.cmp(&right.session_key));
        Ok(sessions)
    }

    pub async fn find_session_by_user_session_id(
        &self,
        user_session_id: &str,
    ) -> Result<Option<SessionEntity>> {
        let sessions = self.list_sessions(None, None).await?;
        Ok(sessions.into_iter().find(|session| {
            session
                .metadata
                .get("user_session_id")
                .and_then(|value| value.as_str())
                == Some(user_session_id)
        }))
    }

    pub async fn update_session_metadata(
        &self,
        session_key: &str,
        metadata: serde_json::Value,
    ) -> Result<SessionEntity> {
        self.ensure_session_active(session_key).await?;
        let now = Self::now_timestamp();
        let mut session = self.require_session(session_key).await?;
        let expected_version = Some(session.version);
        session.metadata = metadata;
        session.updated_at = now;
        session.last_active = now;
        session.version += 1;
        self.store_session_entity(&session, expected_version)
            .await?;
        self.append_session_event(
            session_key,
            SessionEventType::UpdateMetadata,
            serde_json::json!({}),
        )
        .await?;
        Ok(session)
    }

    pub async fn close_session(
        &self,
        session_key: &str,
        reason: Option<String>,
    ) -> Result<SessionStatusState> {
        let session = self.require_session(session_key).await?;
        let now = Self::now_timestamp();
        let mut status = self
            .load_session_status(session_key)
            .await?
            .unwrap_or_else(|| Self::default_session_status(session_key, now));
        if status.status == SessionStatus::Closed {
            return Ok(status);
        }
        let expected_version = Some(status.version);
        status.status = SessionStatus::Closed;
        status.updated_at = now;
        status.version += 1;
        status.reason = reason.clone();
        self.store_session_status(&status, expected_version).await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::Close,
            serde_json::json!({ "reason": reason }),
        )
        .await?;
        self.clear_session_context_references(&session, session_key)
            .await?;
        Ok(status)
    }

    pub async fn close_sessions(
        &self,
        scope: Option<SessionScope>,
        agent_id: Option<&str>,
        reason: Option<String>,
    ) -> Result<Vec<SessionStatusState>> {
        let sessions = self.list_sessions(scope, agent_id).await?;
        let mut statuses = Vec::with_capacity(sessions.len());
        for session in sessions {
            statuses.push(
                self.close_session(&session.session_key, reason.clone())
                    .await?,
            );
        }
        Ok(statuses)
    }

    pub async fn cleanup_sessions(
        &self,
        scope: Option<SessionScope>,
        agent_id: Option<&str>,
    ) -> Result<SessionCleanupReport> {
        let sessions = self.list_sessions(scope.clone(), agent_id).await?;
        let mut report = SessionCleanupReport::default();
        for session in &sessions {
            if let Some(status) = self.get_session_status(&session.session_key).await? {
                report.refreshed_sessions += 1;
                if status.status == SessionStatus::Expired {
                    report.expired_sessions += 1;
                    let cleared = self
                        .clear_session_context_references(session, &session.session_key)
                        .await?;
                    report.cleared_active_session |= cleared.0;
                    report.cleared_auto_session |= cleared.1;
                }
            }
        }
        if let Some(scope) = scope {
            let Some(state) = self
                .get_session_context_state(scope.clone(), agent_id)
                .await?
            else {
                return Ok(report);
            };
            let active_session_active = match state.active_session_key.as_deref() {
                Some(session_key) => self.get_active_session_entity(session_key).await?.is_some(),
                None => true,
            };
            let auto_session_active = match state.auto_session_key.as_deref() {
                Some(session_key) => self.get_active_session_entity(session_key).await?.is_some(),
                None => true,
            };
            if !active_session_active || !auto_session_active {
                self.update_session_context_state(scope, agent_id, |state| {
                    if !active_session_active {
                        state.active_session_key = None;
                    }
                    if !auto_session_active {
                        state.auto_session_key = None;
                    }
                })
                .await?;
                report.cleared_active_session |= !active_session_active;
                report.cleared_auto_session |= !auto_session_active;
            }
        }
        Ok(report)
    }

    pub async fn restart_sessions(
        &self,
        scope: Option<SessionScope>,
        agent_id: Option<&str>,
    ) -> Result<SessionRestartReport> {
        let sessions = self.list_sessions(scope, agent_id).await?;
        let mut instance_ids = Vec::new();
        let mut seen = HashSet::new();
        for session in sessions {
            for instance in self.session_service_instances(&session).await? {
                if seen.insert(instance.instance_id) {
                    self.restart_service(instance.instance_id).await?;
                    instance_ids.push(instance.instance_id);
                }
            }
        }
        Ok(SessionRestartReport {
            restarted_instances: instance_ids,
        })
    }

    pub async fn extend_session(
        &self,
        session_key: &str,
        lease_seconds: i64,
    ) -> Result<SessionEntity> {
        Self::validate_lease_seconds(Some(lease_seconds))?;
        self.ensure_session_active(session_key).await?;

        let now = Self::now_timestamp();
        let mut session = self.require_session(session_key).await?;
        let expected_version = Some(session.version);
        session.lease_seconds = Some(lease_seconds);
        session.expires_at = Some(now + lease_seconds);
        session.updated_at = now;
        session.last_active = now;
        session.version += 1;
        self.store_session_entity(&session, expected_version)
            .await?;
        self.append_session_event(
            session_key,
            SessionEventType::Extend,
            serde_json::json!({ "lease_seconds": lease_seconds }),
        )
        .await?;
        Ok(session)
    }

    pub async fn extend_session_with_retry(
        &self,
        session_key: &str,
        lease_seconds: i64,
        policy: SessionRetryPolicy,
    ) -> Result<SessionEntity> {
        Self::retry_session_write(policy, || async {
            self.extend_session(session_key, lease_seconds).await
        })
        .await
    }

    pub async fn bind_service_to_session(
        &self,
        session_key: &str,
        instance_id: InstanceId,
    ) -> Result<SessionServiceRelation> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let instance = self.require_session_instance(&session, instance_id).await?;
        let now = Self::now_timestamp();
        let loaded_relation = self.load_session_services(session_key).await?;
        let expected_version = loaded_relation.as_ref().map(|relation| relation.version);
        let mut relation = loaded_relation.unwrap_or(SessionServiceRelation {
            session_key: session_key.to_string(),
            services: Vec::new(),
            updated_at: now,
            version: 0,
        });
        if !relation
            .services
            .iter()
            .any(|item| item.instance_id == instance_id)
        {
            relation.services.push(SessionServiceItem {
                instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
                bound_at: now,
            });
        }
        relation.updated_at = now;
        relation.version += 1;
        self.store_session_services(&relation, expected_version)
            .await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::BindService,
            serde_json::json!({
                "instance_id": instance_id,
                "service_name": instance.service_name,
                "scope": instance.scope,
            }),
        )
        .await?;
        Ok(relation)
    }

    pub async fn bind_service_to_session_with_retry(
        &self,
        session_key: &str,
        instance_id: InstanceId,
        policy: SessionRetryPolicy,
    ) -> Result<SessionServiceRelation> {
        Self::retry_session_write(policy, || async {
            self.bind_service_to_session(session_key, instance_id).await
        })
        .await
    }

    pub async fn unbind_service_from_session(
        &self,
        session_key: &str,
        instance_id: InstanceId,
    ) -> Result<SessionServiceRelation> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        self.require_session_instance(&session, instance_id).await?;
        let now = Self::now_timestamp();
        let loaded_relation = self.load_session_services(session_key).await?;
        let expected_version = loaded_relation.as_ref().map(|relation| relation.version);
        let mut relation = loaded_relation.unwrap_or(SessionServiceRelation {
            session_key: session_key.to_string(),
            services: Vec::new(),
            updated_at: now,
            version: 0,
        });
        relation
            .services
            .retain(|item| item.instance_id != instance_id);
        relation.updated_at = now;
        relation.version += 1;
        self.store_session_services(&relation, expected_version)
            .await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::UnbindService,
            serde_json::json!({ "instance_id": instance_id }),
        )
        .await?;
        Ok(relation)
    }

    pub async fn unbind_service_from_session_with_retry(
        &self,
        session_key: &str,
        instance_id: InstanceId,
        policy: SessionRetryPolicy,
    ) -> Result<SessionServiceRelation> {
        Self::retry_session_write(policy, || async {
            self.unbind_service_from_session(session_key, instance_id)
                .await
        })
        .await
    }

    pub async fn list_session_services(
        &self,
        session_key: &str,
    ) -> Result<Vec<SessionServiceItem>> {
        self.require_session(session_key).await?;
        Ok(self
            .load_session_services(session_key)
            .await?
            .map(|relation| relation.services)
            .unwrap_or_default())
    }

    pub async fn set_session_tool_visibility(
        &self,
        session_key: &str,
        selections: Vec<SessionToolSelection>,
    ) -> Result<SessionToolVisibility> {
        self.ensure_session_active(session_key).await?;
        let now = Self::now_timestamp();
        let session = self.require_session(session_key).await?;
        let bound_instance_ids = self
            .session_service_instances(&session)
            .await?
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect::<HashSet<_>>();
        let mut tools = Vec::with_capacity(selections.len());
        for selection in selections {
            if !bound_instance_ids.contains(&selection.instance_id) {
                return Err(StoreError::Other(format!(
                    "instance is not bound to session: session_key={session_key}, instance_id={}",
                    selection.instance_id
                )));
            }
            let instance = self
                .require_session_instance(&session, selection.instance_id)
                .await?;
            instance
                .tools
                .iter()
                .find(|tool| tool.name == selection.tool_name)
                .ok_or_else(|| {
                    StoreError::Other(format!(
                        "Tool not found in instance: instance_id={}, tool_name={}",
                        selection.instance_id, selection.tool_name
                    ))
                })?;
            tools.push(SessionToolItem {
                instance_id: selection.instance_id,
                service_name: instance.service_name,
                scope: instance.scope,
                tool_name: selection.tool_name,
            });
        }
        let loaded_visibility = self.load_session_tool_visibility(session_key).await?;
        let expected_version = loaded_visibility
            .as_ref()
            .map(|visibility| visibility.version);
        let previous_version = expected_version.unwrap_or(0);
        let visibility = SessionToolVisibility {
            session_key: session_key.to_string(),
            mode: ToolVisibilityMode::Allowlist,
            tools,
            updated_at: now,
            version: previous_version + 1,
        };
        self.store_session_tool_visibility(&visibility, expected_version)
            .await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::SetToolVisibility,
            serde_json::json!({ "mode": "allowlist" }),
        )
        .await?;
        Ok(visibility)
    }

    pub async fn list_session_tools(&self, session_key: &str) -> Result<Vec<SessionToolItem>> {
        self.require_session(session_key).await?;
        Ok(self
            .load_session_tool_visibility(session_key)
            .await?
            .map(|visibility| visibility.tools)
            .unwrap_or_default())
    }

    pub async fn get_session_state_value(
        &self,
        session_key: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.require_session(session_key).await?;
        Self::validate_session_state_key(key)?;
        Ok(self
            .load_session_state(session_key)
            .await?
            .and_then(|state| state.values.get(key).cloned()))
    }

    pub async fn list_session_state(&self, session_key: &str) -> Result<SessionStateData> {
        self.require_session(session_key).await?;
        let now = Self::now_timestamp();
        Ok(self
            .load_session_state(session_key)
            .await?
            .unwrap_or_else(|| Self::empty_session_state(session_key, now)))
    }

    pub async fn set_session_state(
        &self,
        session_key: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<SessionStateData> {
        self.ensure_session_active(session_key).await?;
        Self::validate_session_state_key(key)?;
        let now = Self::now_timestamp();
        let loaded_state = self.load_session_state(session_key).await?;
        let expected_version = loaded_state.as_ref().map(|state| state.version);
        let mut state = loaded_state.unwrap_or_else(|| Self::empty_session_state(session_key, now));
        state.values.insert(key.to_string(), value);
        state.updated_at = now;
        state.version += 1;
        self.store_session_state(&state, expected_version).await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::SetState,
            serde_json::json!({ "key": key }),
        )
        .await?;
        Ok(state)
    }

    pub async fn set_session_state_with_retry(
        &self,
        session_key: &str,
        key: &str,
        value: serde_json::Value,
        policy: SessionRetryPolicy,
    ) -> Result<SessionStateData> {
        Self::retry_session_write(policy, || {
            let value = value.clone();
            async move { self.set_session_state(session_key, key, value).await }
        })
        .await
    }

    pub async fn delete_session_state(
        &self,
        session_key: &str,
        key: &str,
    ) -> Result<SessionStateData> {
        self.ensure_session_active(session_key).await?;
        Self::validate_session_state_key(key)?;
        let now = Self::now_timestamp();
        let loaded_state = self.load_session_state(session_key).await?;
        let expected_version = loaded_state.as_ref().map(|state| state.version);
        let mut state = loaded_state.unwrap_or_else(|| Self::empty_session_state(session_key, now));
        state.values.remove(key);
        state.updated_at = now;
        state.version += 1;
        self.store_session_state(&state, expected_version).await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::DeleteState,
            serde_json::json!({ "key": key }),
        )
        .await?;
        Ok(state)
    }

    pub async fn delete_session_state_with_retry(
        &self,
        session_key: &str,
        key: &str,
        policy: SessionRetryPolicy,
    ) -> Result<SessionStateData> {
        Self::retry_session_write(policy, || async {
            self.delete_session_state(session_key, key).await
        })
        .await
    }

    pub async fn clear_session_state(&self, session_key: &str) -> Result<SessionStateData> {
        self.ensure_session_active(session_key).await?;
        let now = Self::now_timestamp();
        let loaded_state = self.load_session_state(session_key).await?;
        let expected_version = loaded_state.as_ref().map(|state| state.version);
        let mut state = loaded_state.unwrap_or_else(|| Self::empty_session_state(session_key, now));
        state.values.clear();
        state.updated_at = now;
        state.version += 1;
        self.store_session_state(&state, expected_version).await?;
        self.touch_session(session_key, now).await?;
        self.append_session_event(
            session_key,
            SessionEventType::ClearState,
            serde_json::json!({}),
        )
        .await?;
        Ok(state)
    }

    pub async fn export_sessions_snapshot(&self) -> Result<serde_json::Value> {
        let snapshot = self.cache.snapshot().await?;
        let mut relations = serde_json::Map::new();
        relations.insert(
            SESSION_SERVICES_RELATION_TYPE.to_string(),
            serde_json::to_value(
                snapshot
                    .relations
                    .get(SESSION_SERVICES_RELATION_TYPE)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(|e| StoreError::Other(e.to_string()))?,
        );
        relations.insert(
            SESSION_TOOLS_RELATION_TYPE.to_string(),
            serde_json::to_value(
                snapshot
                    .relations
                    .get(SESSION_TOOLS_RELATION_TYPE)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(|e| StoreError::Other(e.to_string()))?,
        );
        let mut states = serde_json::Map::new();
        states.insert(
            SESSION_STATUS_STATE_TYPE.to_string(),
            serde_json::to_value(
                snapshot
                    .states
                    .get(SESSION_STATUS_STATE_TYPE)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(|e| StoreError::Other(e.to_string()))?,
        );
        states.insert(
            SESSION_STATE_TYPE.to_string(),
            serde_json::to_value(
                snapshot
                    .states
                    .get(SESSION_STATE_TYPE)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(|e| StoreError::Other(e.to_string()))?,
        );
        states.insert(
            SESSION_CONTEXT_STATE_TYPE.to_string(),
            serde_json::to_value(
                snapshot
                    .states
                    .get(SESSION_CONTEXT_STATE_TYPE)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(|e| StoreError::Other(e.to_string()))?,
        );
        Ok(serde_json::json!({
            "entities": snapshot.entities.get(SESSION_ENTITY_TYPE).cloned().unwrap_or_default(),
            "relations": relations,
            "states": states,
            "events": snapshot.events.get(SESSION_EVENT_TYPE).cloned().unwrap_or_default(),
        }))
    }

    pub async fn import_sessions_snapshot(
        &self,
        snapshot: serde_json::Value,
    ) -> Result<SessionImportReport> {
        let snapshot = Self::validate_sessions_snapshot(snapshot)?;
        let mut report = SessionImportReport::default();
        let mut unchanged_entities = HashSet::new();
        let mut unchanged_service_relations = HashSet::new();
        let mut unchanged_tool_relations = HashSet::new();
        let mut unchanged_status_states = HashSet::new();
        let mut unchanged_session_states = HashSet::new();
        let mut unchanged_context_states = HashSet::new();
        let mut unchanged_events = HashSet::new();

        for (key, _, value) in &snapshot.entities {
            match self.cache.get_entity(SESSION_ENTITY_TYPE, key).await? {
                Some(current) if current == *value => {
                    unchanged_entities.insert(key.clone());
                    report.sessions_unchanged += 1;
                }
                Some(_) => {
                    return Err(Self::session_import_conflict("entity", key));
                }
                None => {}
            }
        }
        for (key, _, value) in &snapshot.service_relations {
            match self
                .cache
                .get_relation(SESSION_SERVICES_RELATION_TYPE, key)
                .await?
            {
                Some(current) if current == *value => {
                    unchanged_service_relations.insert(key.clone());
                    report.session_service_relations_unchanged += 1
                }
                Some(_) => return Err(Self::session_import_conflict("service relation", key)),
                None => {}
            }
        }
        for (key, _, value) in &snapshot.tool_relations {
            match self
                .cache
                .get_relation(SESSION_TOOLS_RELATION_TYPE, key)
                .await?
            {
                Some(current) if current == *value => {
                    unchanged_tool_relations.insert(key.clone());
                    report.session_tool_relations_unchanged += 1;
                }
                Some(_) => return Err(Self::session_import_conflict("tool relation", key)),
                None => {}
            }
        }
        for (key, _, value) in &snapshot.status_states {
            match self.cache.get_state(SESSION_STATUS_STATE_TYPE, key).await? {
                Some(current) if current == *value => {
                    unchanged_status_states.insert(key.clone());
                    report.session_status_states_unchanged += 1;
                }
                Some(_) => return Err(Self::session_import_conflict("status state", key)),
                None => {}
            }
        }
        for (key, _, value) in &snapshot.session_states {
            match self.cache.get_state(SESSION_STATE_TYPE, key).await? {
                Some(current) if current == *value => {
                    unchanged_session_states.insert(key.clone());
                    report.session_state_records_unchanged += 1;
                }
                Some(_) => return Err(Self::session_import_conflict("state", key)),
                None => {}
            }
        }
        for (key, _, value) in &snapshot.context_states {
            match self
                .cache
                .get_state(SESSION_CONTEXT_STATE_TYPE, key)
                .await?
            {
                Some(current) if current == *value => {
                    unchanged_context_states.insert(key.clone());
                    report.session_context_states_unchanged += 1;
                }
                Some(_) => return Err(Self::session_import_conflict("context state", key)),
                None => {}
            }
        }
        for (key, _, value) in &snapshot.events {
            match self.cache.get_event(SESSION_EVENT_TYPE, key).await? {
                Some(current) if current == *value => {
                    unchanged_events.insert(key.clone());
                    report.session_events_unchanged += 1;
                }
                Some(_) => return Err(Self::session_import_conflict("event", key)),
                None => {}
            }
        }

        for (key, _, value) in snapshot.entities {
            if unchanged_entities.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_entity(SESSION_ENTITY_TYPE, &key, None, value)
                .await?;
            report.sessions_imported += 1;
        }
        for (key, _, value) in snapshot.service_relations {
            if unchanged_service_relations.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_relation(SESSION_SERVICES_RELATION_TYPE, &key, None, value)
                .await?;
            report.session_service_relations_imported += 1;
        }
        for (key, _, value) in snapshot.tool_relations {
            if unchanged_tool_relations.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_relation(SESSION_TOOLS_RELATION_TYPE, &key, None, value)
                .await?;
            report.session_tool_relations_imported += 1;
        }
        for (key, _, value) in snapshot.status_states {
            if unchanged_status_states.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_state(SESSION_STATUS_STATE_TYPE, &key, None, value)
                .await?;
            report.session_status_states_imported += 1;
        }
        for (key, _, value) in snapshot.session_states {
            if unchanged_session_states.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_state(SESSION_STATE_TYPE, &key, None, value)
                .await?;
            report.session_state_records_imported += 1;
        }
        for (key, _, value) in snapshot.context_states {
            if unchanged_context_states.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_state(SESSION_CONTEXT_STATE_TYPE, &key, None, value)
                .await?;
            report.session_context_states_imported += 1;
        }
        for (key, _, value) in snapshot.events {
            if unchanged_events.contains(&key) {
                continue;
            }
            self.cache
                .compare_and_put_event(SESSION_EVENT_TYPE, &key, None, value)
                .await?;
            report.session_events_imported += 1;
        }

        Ok(report)
    }

    pub async fn list_tools_in_session(&self, session_key: &str) -> Result<Vec<ScopedToolEntry>> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let mut tools = self.collect_session_tool_entries(&session).await?;
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(tools)
    }

    pub async fn call_tool_in_session(
        &self,
        session_key: &str,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let bound = self
            .session_service_instances(&session)
            .await?
            .into_iter()
            .any(|instance| instance.instance_id == instance_id);
        let visibility = self
            .load_session_tool_visibility(&session.session_key)
            .await?;
        if !bound || !Self::session_tool_allowed(&visibility, instance_id, tool_name) {
            let error = StoreError::Other(format!(
                "tool is not available in session: session_key={session_key}, instance_id={instance_id}, tool_name={tool_name}"
            ));
            self.append_session_event(
                session_key,
                SessionEventType::CallDenied,
                serde_json::json!({
                    "instance_id": instance_id,
                    "tool_name": tool_name,
                    "reason": error.to_string(),
                }),
            )
            .await?;
            return Err(error);
        }
        self.call_tool(instance_id, tool_name, args).await
    }

    pub async fn list_resources_in_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let mut resources = Vec::new();
        for instance in self.session_service_instances(&session).await? {
            let mut service_resources = self.list_resources(instance.instance_id).await?;
            service_resources.sort_by(|left, right| left.uri.cmp(&right.uri));
            for resource in service_resources {
                let mut value = serde_json::to_value(resource)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
                if let serde_json::Value::Object(object) = &mut value {
                    object.insert(
                        "instance_id".to_string(),
                        serde_json::json!(instance.instance_id),
                    );
                    object.insert(
                        "service_name".to_string(),
                        serde_json::json!(instance.service_name),
                    );
                    object.insert("scope".to_string(), serde_json::json!(instance.scope));
                }
                resources.push(value);
            }
        }
        Ok(resources)
    }

    pub async fn list_resource_templates_in_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let mut templates = Vec::new();
        for instance in self.session_service_instances(&session).await? {
            let mut service_templates = self.list_resource_templates(instance.instance_id).await?;
            service_templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
            for template in service_templates {
                let mut value = serde_json::to_value(template)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
                if let serde_json::Value::Object(object) = &mut value {
                    object.insert(
                        "instance_id".to_string(),
                        serde_json::json!(instance.instance_id),
                    );
                    object.insert(
                        "service_name".to_string(),
                        serde_json::json!(instance.service_name),
                    );
                    object.insert("scope".to_string(), serde_json::json!(instance.scope));
                }
                templates.push(value);
            }
        }
        Ok(templates)
    }

    pub async fn read_resource_in_session(
        &self,
        session_key: &str,
        uri: &str,
        instance_id: InstanceId,
    ) -> Result<serde_json::Value> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        self.require_bound_session_instance(&session, instance_id)
            .await?;
        self.read_resource(instance_id, uri).await
    }

    pub async fn list_prompts_in_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        let mut prompts = Vec::new();
        for instance in self.session_service_instances(&session).await? {
            let mut service_prompts = self.list_prompts(instance.instance_id).await?;
            service_prompts.sort_by(|left, right| left.name.cmp(&right.name));
            for prompt in service_prompts {
                let mut value = serde_json::to_value(prompt)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
                if let serde_json::Value::Object(object) = &mut value {
                    object.insert(
                        "instance_id".to_string(),
                        serde_json::json!(instance.instance_id),
                    );
                    object.insert(
                        "service_name".to_string(),
                        serde_json::json!(instance.service_name),
                    );
                    object.insert("scope".to_string(), serde_json::json!(instance.scope));
                }
                prompts.push(value);
            }
        }
        Ok(prompts)
    }

    pub async fn get_prompt_in_session(
        &self,
        session_key: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
        instance_id: InstanceId,
    ) -> Result<serde_json::Value> {
        self.ensure_session_active(session_key).await?;
        let session = self.require_session(session_key).await?;
        self.require_bound_session_instance(&session, instance_id)
            .await?;
        self.get_prompt(instance_id, prompt_name, arguments).await
    }

    async fn ensure_session_active(&self, session_key: &str) -> Result<()> {
        let session = self.require_session(session_key).await?;
        let now = Self::now_timestamp();
        let mut status = self
            .load_session_status(session_key)
            .await?
            .unwrap_or_else(|| Self::default_session_status(session_key, now));
        if status.status == SessionStatus::Active {
            if let Some(expires_at) = session.expires_at {
                if expires_at <= now {
                    let expected_version = Some(status.version);
                    status.status = SessionStatus::Expired;
                    status.updated_at = now;
                    status.version += 1;
                    status.reason = Some("lease expired".to_string());
                    self.store_session_status(&status, expected_version).await?;
                    self.append_session_event(
                        session_key,
                        SessionEventType::Expire,
                        serde_json::json!({ "expires_at": expires_at }),
                    )
                    .await?;
                }
            }
        }
        if status.status != SessionStatus::Active {
            return Err(StoreError::Other(format!(
                "Session is not active: session_key={session_key}, status={:?}",
                status.status
            )));
        }
        Ok(())
    }

    async fn collect_session_tool_entries(
        &self,
        session: &SessionEntity,
    ) -> Result<Vec<ScopedToolEntry>> {
        let visibility = self
            .load_session_tool_visibility(&session.session_key)
            .await?;
        let mut entries = Vec::new();
        for instance in self.session_service_instances(session).await? {
            let mut service_tools = instance.tools.clone();
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                if !Self::session_tool_allowed(&visibility, instance.instance_id, &tool.name) {
                    continue;
                }
                let transformed = self
                    .apply_tool_transform(
                        instance.instance_id,
                        &tool.name,
                        tool.name.clone(),
                        tool.description,
                        tool.input_schema,
                    )
                    .await?;
                entries.push(Self::scoped_tool_entry(
                    transformed.display_name,
                    tool.name,
                    instance.instance_id,
                    instance.service_name.clone(),
                    instance.scope.clone(),
                    tool.title,
                    transformed.description,
                    transformed.input_schema,
                    tool.output_schema,
                    tool.annotations,
                    tool.meta,
                ));
            }
        }
        Ok(entries)
    }

    async fn session_service_instances(
        &self,
        session: &SessionEntity,
    ) -> Result<Vec<ServiceInstance>> {
        let bound = self.load_session_services(&session.session_key).await?;
        let mut instances = if let Some(relation) = bound {
            let mut instances = Vec::with_capacity(relation.services.len());
            for item in relation.services {
                let instance = self
                    .require_session_instance(session, item.instance_id)
                    .await?;
                if instance.service_name != item.service_name || instance.scope != item.scope {
                    return Err(StoreError::Other(format!(
                        "session service relation does not match instance: session_key={}, instance_id={}",
                        session.session_key, item.instance_id
                    )));
                }
                instances.push(instance);
            }
            instances
        } else {
            self.list_scope_instances(&Self::session_scope_ref(session)?)
                .await?
        };
        instances.sort_by_key(|instance| instance.instance_id);
        instances.dedup_by_key(|instance| instance.instance_id);
        Ok(instances)
    }

    fn session_tool_allowed(
        visibility: &Option<SessionToolVisibility>,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> bool {
        let Some(visibility) = visibility else {
            return true;
        };
        match visibility.mode {
            ToolVisibilityMode::Allowlist => visibility
                .tools
                .iter()
                .any(|item| item.instance_id == instance_id && item.tool_name == tool_name),
        }
    }

    async fn require_session_instance(
        &self,
        session: &SessionEntity,
        instance_id: InstanceId,
    ) -> Result<ServiceInstance> {
        self.refresh_from_db_if_needed().await?;
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let expected_scope = Self::session_scope_ref(session)?;
        if instance.scope != expected_scope {
            return Err(StoreError::Other(format!(
                "instance does not belong to session scope: session_key={}, instance_id={instance_id}",
                session.session_key
            )));
        }
        Ok(instance)
    }

    async fn require_bound_session_instance(
        &self,
        session: &SessionEntity,
        instance_id: InstanceId,
    ) -> Result<ServiceInstance> {
        self.session_service_instances(session)
            .await?
            .into_iter()
            .find(|instance| instance.instance_id == instance_id)
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "instance is not bound to session: session_key={}, instance_id={instance_id}",
                    session.session_key
                ))
            })
    }

    fn session_scope_ref(session: &SessionEntity) -> Result<ScopeRef> {
        match &session.scope {
            SessionScope::Store => Ok(ScopeRef::Store),
            SessionScope::Agent => {
                let agent_id = session.agent_id.clone().ok_or_else(|| {
                    StoreError::Other("agent-scoped session missing agent_id".to_string())
                })?;
                Ok(ScopeRef::Agent { agent_id })
            }
        }
    }

    fn validate_session_instance_identity(
        session: &SessionEntity,
        instance_id: InstanceId,
        service_name: &str,
        scope: &ScopeRef,
        kind: &str,
    ) -> Result<()> {
        let expected_scope = Self::session_scope_ref(session)?;
        if scope != &expected_scope {
            return Err(StoreError::Other(format!(
                "{kind} scope does not match session: session_key={}, instance_id={instance_id}",
                session.session_key
            )));
        }
        let expected_instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        if instance_id != expected_instance_id {
            return Err(StoreError::Other(format!(
                "{kind} instance identity mismatch: session_key={}, instance_id={instance_id}, expected_instance_id={expected_instance_id}",
                session.session_key
            )));
        }
        Ok(())
    }

    async fn require_session(&self, session_key: &str) -> Result<SessionEntity> {
        self.get_session(session_key).await?.ok_or_else(|| {
            StoreError::Other(format!("Session not found: session_key={session_key}"))
        })
    }

    async fn ensure_session_matches_context(
        &self,
        scope: &SessionScope,
        agent_id: Option<&str>,
        session_key: &str,
    ) -> Result<()> {
        let session = self.require_session(session_key).await?;
        if &session.scope != scope || session.agent_id.as_deref() != agent_id {
            return Err(StoreError::Other(format!(
                "Session does not belong to requested context: session_key={session_key}"
            )));
        }
        Ok(())
    }

    async fn get_active_session_entity(&self, session_key: &str) -> Result<Option<SessionEntity>> {
        let Some(session) = self.get_session(session_key).await? else {
            return Ok(None);
        };
        let Some(status) = self.get_session_status(session_key).await? else {
            return Ok(None);
        };
        if status.status == SessionStatus::Active {
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    async fn clear_session_context_references(
        &self,
        session: &SessionEntity,
        session_key: &str,
    ) -> Result<(bool, bool)> {
        let scope = session.scope.clone();
        let agent_id = session.agent_id.as_deref();
        let Some(state) = self
            .get_session_context_state(scope.clone(), agent_id)
            .await?
        else {
            return Ok((false, false));
        };
        let clear_active = state.active_session_key.as_deref() == Some(session_key);
        let clear_auto = state.auto_session_key.as_deref() == Some(session_key);
        if !clear_active && !clear_auto {
            return Ok((false, false));
        }
        self.update_session_context_state(scope, agent_id, |state| {
            if clear_active {
                state.active_session_key = None;
            }
            if clear_auto {
                state.auto_session_key = None;
            }
        })
        .await?;
        Ok((clear_active, clear_auto))
    }

    async fn load_session_context_state(
        &self,
        context_key: &str,
    ) -> Result<Option<SessionContextState>> {
        match self
            .cache
            .get_state(SESSION_CONTEXT_STATE_TYPE, context_key)
            .await?
        {
            Some(value) => serde_json::from_value(value)
                .map(Some)
                .map_err(|e| StoreError::Other(e.to_string())),
            None => Ok(None),
        }
    }

    async fn update_session_context_state<F>(
        &self,
        scope: SessionScope,
        agent_id: Option<&str>,
        mut update: F,
    ) -> Result<SessionContextState>
    where
        F: FnMut(&mut SessionContextState),
    {
        let context_key = Self::build_session_context_key(&scope, agent_id)?;
        for _ in 0..3 {
            let now = Self::now_timestamp();
            let current = self.load_session_context_state(&context_key).await?;
            let expected_version = current.as_ref().map(|state| state.version);
            let mut state = current.unwrap_or_else(|| SessionContextState {
                context_key: context_key.clone(),
                active_session_key: None,
                auto_session_key: None,
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
                    SESSION_CONTEXT_STATE_TYPE,
                    &context_key,
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
            "session context state conflict after retries: context_key={context_key}"
        ))))
    }

    async fn touch_session(&self, session_key: &str, now: i64) -> Result<()> {
        for _ in 0..3 {
            let mut session = self.require_session(session_key).await?;
            let expected_version = Some(session.version);
            session.updated_at = now;
            session.last_active = now;
            session.version += 1;
            match self.store_session_entity(&session, expected_version).await {
                Ok(()) => return Ok(()),
                Err(error) if Self::is_cache_conflict(&error) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(StoreError::Cache(CacheError::Conflict(format!(
            "session touch conflict after retries: session_key={session_key}"
        ))))
    }

    async fn retry_session_write<T, Op, Fut>(
        policy: SessionRetryPolicy,
        mut operation: Op,
    ) -> Result<T>
    where
        Op: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let max_attempts = policy.max_attempts.max(1);
        let mut last_conflict: Option<StoreError> = None;
        for attempt in 0..max_attempts {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(error) if Self::is_cache_conflict(&error) => {
                    last_conflict = Some(error);
                    if attempt + 1 < max_attempts && policy.delay_millis > 0 {
                        tokio::time::sleep(Duration::from_millis(policy.delay_millis)).await;
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_conflict.unwrap_or_else(|| {
            StoreError::Cache(CacheError::Conflict(
                "session write conflict after retries".to_string(),
            ))
        }))
    }

    async fn store_session_entity(
        &self,
        session: &SessionEntity,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_entity(
                SESSION_ENTITY_TYPE,
                &session.session_key,
                expected_version,
                serde_json::to_value(session).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    async fn load_session_status(&self, session_key: &str) -> Result<Option<SessionStatusState>> {
        let Some(value) = self
            .cache
            .get_state(SESSION_STATUS_STATE_TYPE, session_key)
            .await?
        else {
            return Ok(None);
        };
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| StoreError::Other(format!("Session status deserialization failed: {e}")))
    }

    async fn store_session_status(
        &self,
        status: &SessionStatusState,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_state(
                SESSION_STATUS_STATE_TYPE,
                &status.session_key,
                expected_version,
                serde_json::to_value(status).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    async fn load_session_state(&self, session_key: &str) -> Result<Option<SessionStateData>> {
        let Some(value) = self
            .cache
            .get_state(SESSION_STATE_TYPE, session_key)
            .await?
        else {
            return Ok(None);
        };
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| StoreError::Other(format!("Session state deserialization failed: {e}")))
    }

    async fn store_session_state(
        &self,
        state: &SessionStateData,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_state(
                SESSION_STATE_TYPE,
                &state.session_key,
                expected_version,
                serde_json::to_value(state).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    async fn load_session_services(
        &self,
        session_key: &str,
    ) -> Result<Option<SessionServiceRelation>> {
        let Some(value) = self
            .cache
            .get_relation(SESSION_SERVICES_RELATION_TYPE, session_key)
            .await?
        else {
            return Ok(None);
        };
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| StoreError::Other(format!("Session services deserialization failed: {e}")))
    }

    async fn store_session_services(
        &self,
        relation: &SessionServiceRelation,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_relation(
                SESSION_SERVICES_RELATION_TYPE,
                &relation.session_key,
                expected_version,
                serde_json::to_value(relation).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    async fn load_session_tool_visibility(
        &self,
        session_key: &str,
    ) -> Result<Option<SessionToolVisibility>> {
        let Some(value) = self
            .cache
            .get_relation(SESSION_TOOLS_RELATION_TYPE, session_key)
            .await?
        else {
            return Ok(None);
        };
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| StoreError::Other(format!("Session tools deserialization failed: {e}")))
    }

    async fn store_session_tool_visibility(
        &self,
        visibility: &SessionToolVisibility,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_relation(
                SESSION_TOOLS_RELATION_TYPE,
                &visibility.session_key,
                expected_version,
                serde_json::to_value(visibility).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    async fn append_session_event(
        &self,
        session_key: &str,
        event_type: SessionEventType,
        payload: serde_json::Value,
    ) -> Result<()> {
        let occurred_at = Self::now_timestamp();
        let key = format!("{session_key}:{occurred_at}:{}", Uuid::new_v4());
        let event = SessionEvent {
            session_key: session_key.to_string(),
            event_type,
            occurred_at,
            payload,
        };
        self.cache
            .put_event(
                SESSION_EVENT_TYPE,
                &key,
                serde_json::to_value(event).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await
            .map_err(Into::into)
    }

    fn decode_session_entity(value: serde_json::Value) -> Result<SessionEntity> {
        serde_json::from_value(value)
            .map_err(|e| StoreError::Other(format!("Session deserialization failed: {e}")))
    }

    fn validate_sessions_snapshot(snapshot: serde_json::Value) -> Result<ValidatedSessionSnapshot> {
        let root = snapshot.as_object().ok_or_else(|| {
            StoreError::Other("session snapshot must be a JSON object".to_string())
        })?;
        let entities_map = Self::required_object(root, "entities")?;
        let relations_map = Self::required_object(root, "relations")?;
        let states_map = Self::required_object(root, "states")?;
        let service_relations_map =
            Self::required_object(relations_map, SESSION_SERVICES_RELATION_TYPE)?;
        let tool_relations_map = Self::required_object(relations_map, SESSION_TOOLS_RELATION_TYPE)?;
        let status_states_map = Self::required_object(states_map, SESSION_STATUS_STATE_TYPE)?;
        let session_states_map = Self::optional_object(states_map, SESSION_STATE_TYPE)?;
        let context_states_map = Self::optional_object(states_map, SESSION_CONTEXT_STATE_TYPE)?;
        let events_map = Self::required_object(root, "events")?;

        let mut session_keys = HashSet::with_capacity(entities_map.len());
        let mut session_entities = HashMap::with_capacity(entities_map.len());
        let mut entities = Vec::with_capacity(entities_map.len());
        for (key, value) in entities_map {
            let entity: SessionEntity = Self::decode_import_value("session entity", key, value)?;
            if entity.session_key != *key {
                return Err(StoreError::Other(format!(
                    "session entity key mismatch: key={key}, session_key={}",
                    entity.session_key
                )));
            }
            let expected_key = Self::build_session_key(
                &entity.scope,
                entity.agent_id.as_deref(),
                &entity.session_id,
            )?;
            if expected_key != *key {
                return Err(StoreError::Other(format!(
                    "session entity fields do not match key: key={key}, expected={expected_key}"
                )));
            }
            Self::validate_lease_seconds(entity.lease_seconds)?;
            session_keys.insert(key.clone());
            session_entities.insert(key.clone(), entity.clone());
            entities.push((
                key.clone(),
                entity.clone(),
                Self::canonical_value("session entity", key, &entity)?,
            ));
        }

        let mut service_relations = Vec::with_capacity(service_relations_map.len());
        for (key, value) in service_relations_map {
            let relation: SessionServiceRelation =
                Self::decode_import_value("session service relation", key, value)?;
            Self::validate_session_key_reference(
                &session_keys,
                key,
                &relation.session_key,
                "session service relation",
            )?;
            let session = session_entities
                .get(key)
                .expect("validated session key must have an entity");
            for item in &relation.services {
                Self::validate_session_instance_identity(
                    session,
                    item.instance_id,
                    &item.service_name,
                    &item.scope,
                    "session service relation",
                )?;
            }
            service_relations.push((
                key.clone(),
                relation.clone(),
                Self::canonical_value("session service relation", key, &relation)?,
            ));
        }

        let mut tool_relations = Vec::with_capacity(tool_relations_map.len());
        for (key, value) in tool_relations_map {
            let visibility: SessionToolVisibility =
                Self::decode_import_value("session tool relation", key, value)?;
            Self::validate_session_key_reference(
                &session_keys,
                key,
                &visibility.session_key,
                "session tool relation",
            )?;
            let session = session_entities
                .get(key)
                .expect("validated session key must have an entity");
            for item in &visibility.tools {
                Self::validate_session_instance_identity(
                    session,
                    item.instance_id,
                    &item.service_name,
                    &item.scope,
                    "session tool relation",
                )?;
            }
            tool_relations.push((
                key.clone(),
                visibility.clone(),
                Self::canonical_value("session tool relation", key, &visibility)?,
            ));
        }

        let mut status_states = Vec::with_capacity(status_states_map.len());
        for (key, value) in status_states_map {
            let status: SessionStatusState =
                Self::decode_import_value("session status state", key, value)?;
            Self::validate_session_key_reference(
                &session_keys,
                key,
                &status.session_key,
                "session status state",
            )?;
            status_states.push((
                key.clone(),
                status.clone(),
                Self::canonical_value("session status state", key, &status)?,
            ));
        }

        let mut session_states =
            Vec::with_capacity(session_states_map.map_or(0, |items| items.len()));
        if let Some(session_states_map) = session_states_map {
            for (key, value) in session_states_map {
                let state: SessionStateData =
                    Self::decode_import_value("session state", key, value)?;
                Self::validate_session_key_reference(
                    &session_keys,
                    key,
                    &state.session_key,
                    "session state",
                )?;
                for state_key in state.values.keys() {
                    Self::validate_session_state_key(state_key)?;
                }
                session_states.push((
                    key.clone(),
                    state.clone(),
                    Self::canonical_value("session state", key, &state)?,
                ));
            }
        }

        let mut context_states =
            Vec::with_capacity(context_states_map.map_or(0, |items| items.len()));
        if let Some(context_states_map) = context_states_map {
            for (key, value) in context_states_map {
                let state: SessionContextState =
                    Self::decode_import_value("session context state", key, value)?;
                if state.context_key != *key {
                    return Err(StoreError::Other(format!(
                        "session context state key mismatch: key={key}, context_key={}",
                        state.context_key
                    )));
                }
                Self::validate_session_context_state_references(&session_entities, key, &state)?;
                context_states.push((
                    key.clone(),
                    state.clone(),
                    Self::canonical_value("session context state", key, &state)?,
                ));
            }
        }

        let mut events = Vec::with_capacity(events_map.len());
        for (key, value) in events_map {
            let event: SessionEvent = Self::decode_import_value("session event", key, value)?;
            if !session_keys.contains(&event.session_key) {
                return Err(StoreError::Other(format!(
                    "session event references missing session: key={key}, session_key={}",
                    event.session_key
                )));
            }
            let event_prefix = format!("{}:", event.session_key);
            if !key.starts_with(&event_prefix) {
                return Err(StoreError::Other(format!(
                    "session event key mismatch: key={key}, session_key={}",
                    event.session_key
                )));
            }
            events.push((
                key.clone(),
                event.clone(),
                Self::canonical_value("session event", key, &event)?,
            ));
        }

        Ok(ValidatedSessionSnapshot {
            entities,
            service_relations,
            tool_relations,
            status_states,
            session_states,
            context_states,
            events,
        })
    }

    fn validate_session_context_state_references(
        session_entities: &HashMap<String, SessionEntity>,
        key: &str,
        state: &SessionContextState,
    ) -> Result<()> {
        if let Some(session_key) = state.active_session_key.as_deref() {
            Self::validate_session_context_key_reference(
                session_entities,
                key,
                session_key,
                "session context active session",
                state,
            )?;
        }
        if let Some(session_key) = state.auto_session_key.as_deref() {
            Self::validate_session_context_key_reference(
                session_entities,
                key,
                session_key,
                "session context auto session",
                state,
            )?;
        }
        Ok(())
    }

    fn validate_session_context_key_reference(
        session_entities: &HashMap<String, SessionEntity>,
        key: &str,
        session_key: &str,
        kind: &str,
        state: &SessionContextState,
    ) -> Result<()> {
        let session = session_entities.get(session_key).ok_or_else(|| {
            StoreError::Other(format!(
                "{kind} references missing session: key={key}, session_key={session_key}"
            ))
        })?;
        let expected_context_key =
            Self::build_session_context_key(&session.scope, session.agent_id.as_deref())?;
        if expected_context_key != state.context_key {
            return Err(StoreError::Other(format!(
                "{kind} references session outside context: key={key}, session_key={session_key}, expected_context_key={expected_context_key}"
            )));
        }
        Ok(())
    }

    fn required_object<'a>(
        object: &'a serde_json::Map<String, serde_json::Value>,
        field: &str,
    ) -> Result<&'a serde_json::Map<String, serde_json::Value>> {
        object
            .get(field)
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                StoreError::Other(format!("session snapshot field must be an object: {field}"))
            })
    }

    fn optional_object<'a>(
        object: &'a serde_json::Map<String, serde_json::Value>,
        field: &str,
    ) -> Result<Option<&'a serde_json::Map<String, serde_json::Value>>> {
        match object.get(field) {
            Some(value) => value.as_object().map(Some).ok_or_else(|| {
                StoreError::Other(format!("session snapshot field must be an object: {field}"))
            }),
            None => Ok(None),
        }
    }

    fn decode_import_value<T>(kind: &str, key: &str, value: &serde_json::Value) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(value.clone()).map_err(|e| {
            StoreError::Other(format!(
                "{kind} import deserialization failed: key={key}, error={e}"
            ))
        })
    }

    fn canonical_value<T>(kind: &str, key: &str, value: &T) -> Result<serde_json::Value>
    where
        T: Serialize,
    {
        serde_json::to_value(value).map_err(|e| {
            StoreError::Other(format!(
                "{kind} import serialization failed: key={key}, error={e}"
            ))
        })
    }

    fn validate_session_key_reference(
        session_keys: &HashSet<String>,
        key: &str,
        session_key: &str,
        kind: &str,
    ) -> Result<()> {
        if key != session_key {
            return Err(StoreError::Other(format!(
                "{kind} key mismatch: key={key}, session_key={session_key}"
            )));
        }
        if !session_keys.contains(session_key) {
            return Err(StoreError::Other(format!(
                "{kind} references missing session: key={key}, session_key={session_key}"
            )));
        }
        Ok(())
    }

    fn session_import_conflict(kind: &str, key: &str) -> StoreError {
        StoreError::Cache(CacheError::Conflict(format!(
            "session import conflict: kind={kind}, key={key}"
        )))
    }

    fn default_session_status(session_key: &str, now: i64) -> SessionStatusState {
        SessionStatusState {
            session_key: session_key.to_string(),
            status: SessionStatus::Active,
            updated_at: now,
            version: 0,
            reason: None,
        }
    }

    fn empty_session_state(session_key: &str, now: i64) -> SessionStateData {
        SessionStateData {
            session_key: session_key.to_string(),
            values: serde_json::Map::new(),
            updated_at: now,
            version: 0,
        }
    }

    fn validate_session_id(value: &str) -> Result<()> {
        if value.trim().is_empty() {
            return Err(StoreError::Other("session id cannot be empty".to_string()));
        }
        if value.contains(':') {
            return Err(StoreError::Other(
                "session id and agent id cannot contain ':'".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_session_state_key(value: &str) -> Result<()> {
        if value.trim().is_empty() {
            return Err(StoreError::Other(
                "session state key cannot be empty".to_string(),
            ));
        }
        if value.contains(':') {
            return Err(StoreError::Other(
                "session state key cannot contain ':'".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_lease_seconds(value: Option<i64>) -> Result<()> {
        if let Some(value) = value {
            if value <= 0 {
                return Err(StoreError::Other(
                    "lease_seconds must be greater than zero".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn is_cache_conflict(error: &StoreError) -> bool {
        matches!(error, StoreError::Cache(CacheError::Conflict(_)))
    }
}

#[cfg(test)]
mod tests;
