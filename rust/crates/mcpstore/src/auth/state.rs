use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rmcp::transport::auth::{AuthError as RmcpAuthError, StateStore, StoredAuthorizationState};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{AuthCredentialKey, AuthError, SystemKeyring};

const OAUTH_STATE_SERVICE: &str = "mcpstore.oauth.authorization-state";
const DEFAULT_STATE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Default, Serialize, Deserialize)]
struct AuthorizationStates {
    states: HashMap<String, StoredAuthorizationState>,
}

pub struct KeyringStateStore {
    keyring: SystemKeyring,
    account: String,
    ttl: Duration,
    lock: Mutex<()>,
}

impl std::fmt::Debug for KeyringStateStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyringStateStore")
            .field("account", &self.account)
            .field("ttl", &self.ttl)
            .finish()
    }
}

impl KeyringStateStore {
    pub fn new(key: &AuthCredentialKey) -> Result<Self, AuthError> {
        Ok(Self::with_keyring_and_ttl(
            key,
            SystemKeyring::new()?,
            DEFAULT_STATE_TTL,
        ))
    }

    pub(crate) fn with_keyring_and_ttl(
        key: &AuthCredentialKey,
        keyring: SystemKeyring,
        ttl: Duration,
    ) -> Self {
        Self {
            keyring,
            account: key.storage_id(),
            ttl,
            lock: Mutex::new(()),
        }
    }

    pub async fn clear(&self) -> Result<(), AuthError> {
        let _guard = self.lock.lock().await;
        self.keyring
            .delete(OAUTH_STATE_SERVICE, self.account.clone())
            .await
    }

    pub async fn purge_expired(&self) -> Result<usize, AuthError> {
        let _guard = self.lock.lock().await;
        let mut states = self.load_states().await?;
        let removed = purge_expired_states(&mut states, self.ttl, now_seconds());
        if removed > 0 {
            self.save_states(&states).await?;
        }
        Ok(removed)
    }

    async fn load_states(&self) -> Result<AuthorizationStates, AuthError> {
        let Some(bytes) = self
            .keyring
            .load(OAUTH_STATE_SERVICE, self.account.clone())
            .await?
        else {
            return Ok(AuthorizationStates::default());
        };
        serde_json::from_slice(&bytes).map_err(|_| AuthError::InvalidStoredData)
    }

    async fn save_states(&self, states: &AuthorizationStates) -> Result<(), AuthError> {
        if states.states.is_empty() {
            return self
                .keyring
                .delete(OAUTH_STATE_SERVICE, self.account.clone())
                .await;
        }
        let bytes = serde_json::to_vec(states).map_err(|_| AuthError::InvalidStoredData)?;
        self.keyring
            .save(OAUTH_STATE_SERVICE, self.account.clone(), bytes)
            .await
    }
}

#[async_trait]
impl StateStore for KeyringStateStore {
    async fn save(
        &self,
        csrf_token: &str,
        state: StoredAuthorizationState,
    ) -> Result<(), RmcpAuthError> {
        let _guard = self.lock.lock().await;
        let mut states = self.load_states().await.map_err(to_rmcp_error)?;
        purge_expired_states(&mut states, self.ttl, now_seconds());
        states.states.insert(csrf_token.to_string(), state);
        self.save_states(&states).await.map_err(to_rmcp_error)
    }

    async fn load(
        &self,
        csrf_token: &str,
    ) -> Result<Option<StoredAuthorizationState>, RmcpAuthError> {
        let _guard = self.lock.lock().await;
        let mut states = self.load_states().await.map_err(to_rmcp_error)?;
        let removed = purge_expired_states(&mut states, self.ttl, now_seconds());
        let state = states.states.get(csrf_token).cloned();
        if removed > 0 {
            self.save_states(&states).await.map_err(to_rmcp_error)?;
        }
        Ok(state)
    }

    async fn delete(&self, csrf_token: &str) -> Result<(), RmcpAuthError> {
        let _guard = self.lock.lock().await;
        let mut states = self.load_states().await.map_err(to_rmcp_error)?;
        if states.states.remove(csrf_token).is_some() {
            self.save_states(&states).await.map_err(to_rmcp_error)?;
        }
        Ok(())
    }
}

fn purge_expired_states(states: &mut AuthorizationStates, ttl: Duration, now: u64) -> usize {
    let before = states.states.len();
    states
        .states
        .retain(|_, state| now.saturating_sub(state.created_at) < ttl.as_secs());
    before - states.states.len()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn to_rmcp_error(error: AuthError) -> RmcpAuthError {
    RmcpAuthError::InternalError(error.to_string())
}
