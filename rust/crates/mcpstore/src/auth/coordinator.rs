use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::identity::InstanceId;

use super::{
    AuthConfig, AuthCredentialKey, AuthError, AuthStatus, KeyringClientSecretStore,
    KeyringCredentialStore, KeyringStateStore, SystemKeyring,
};

pub struct AuthCoordinator {
    keyring: SystemKeyring,
    statuses: Arc<RwLock<HashMap<InstanceId, AuthStatus>>>,
}

impl std::fmt::Debug for AuthCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AuthCoordinator")
    }
}

impl AuthCoordinator {
    pub fn new() -> Result<Self, AuthError> {
        Ok(Self {
            keyring: SystemKeyring::new()?,
            statuses: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn status(&self, instance_id: InstanceId) -> AuthStatus {
        self.statuses
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap_or(AuthStatus::NotRequired)
    }

    pub async fn set_status(&self, instance_id: InstanceId, status: AuthStatus) {
        self.statuses.write().await.insert(instance_id, status);
    }

    pub async fn initialize_status(&self, instance_id: InstanceId, auth: &AuthConfig) {
        let mut statuses = self.statuses.write().await;
        if auth.is_none() {
            statuses.insert(instance_id, AuthStatus::NotRequired);
            return;
        }
        match statuses.get(&instance_id) {
            None | Some(AuthStatus::NotRequired) => {
                statuses.insert(instance_id, AuthStatus::Unauthenticated);
            }
            Some(_) => {}
        }
    }

    pub async fn remove_status(&self, instance_id: InstanceId) {
        self.statuses.write().await.remove(&instance_id);
    }

    pub async fn clear_statuses(&self) {
        self.statuses.write().await.clear();
    }

    pub async fn retain_statuses(&self, instance_ids: &HashSet<InstanceId>) {
        self.statuses
            .write()
            .await
            .retain(|instance_id, _| instance_ids.contains(instance_id));
    }

    pub fn credential_store(&self, key: &AuthCredentialKey) -> KeyringCredentialStore {
        KeyringCredentialStore::with_keyring(key, self.keyring.clone())
    }

    pub fn client_secret_store(&self, key: &AuthCredentialKey) -> KeyringClientSecretStore {
        KeyringClientSecretStore::with_keyring(key, self.keyring.clone())
    }

    pub fn state_store(&self, key: &AuthCredentialKey, ttl: Duration) -> KeyringStateStore {
        KeyringStateStore::with_keyring_and_ttl(key, self.keyring.clone(), ttl)
    }
}
