use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use rmcp::transport::auth::{
    AuthError as RmcpAuthError, AuthorizationCallback, AuthorizationManager, AuthorizationSession,
    ClientCredentialsConfig, CredentialStore, JwtSigningAlgorithm as RmcpJwtSigningAlgorithm,
    OAuthClientConfig, OAuthState,
};
use tokio::sync::{Mutex, RwLock};

#[cfg(test)]
use rmcp::transport::auth::OAuthHttpClient;

use crate::identity::InstanceId;

use super::{
    AuthConfig, AuthCredentialKey, AuthError, AuthFlow, AuthStatus, AuthStatusView,
    AuthorizationStart, ClientCredentialsAuthMethod, ClientSecret, JwtSigningAlgorithm,
    KeyringClientSecretStore, KeyringCredentialStore, KeyringPrivateKeyStore, KeyringStateStore,
    PrivateKey, SystemKeyring,
};

const AUTHORIZATION_STATE_TTL: Duration = Duration::from_secs(10 * 60);
const DYNAMIC_CLIENT_IDENTITY: &str = "dynamic-client-registration";

#[derive(Clone)]
pub struct AuthCoordinator {
    keyring: SystemKeyring,
    statuses: Arc<RwLock<HashMap<InstanceId, AuthStatus>>>,
    required_scopes: Arc<RwLock<HashMap<InstanceId, String>>>,
    refresh_locks: Arc<Mutex<HashMap<InstanceId, Arc<Mutex<()>>>>>,
    sessions: Arc<Mutex<HashMap<InstanceId, AuthorizationSession>>>,
    #[cfg(test)]
    oauth_http_client: Option<Arc<dyn OAuthHttpClient>>,
}

impl std::fmt::Debug for AuthCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AuthCoordinator")
    }
}

impl AuthCoordinator {
    pub fn new() -> Result<Self, AuthError> {
        Self::with_keyring(SystemKeyring::new()?)
    }

    fn with_keyring(keyring: SystemKeyring) -> Result<Self, AuthError> {
        Ok(Self {
            keyring,
            statuses: Arc::new(RwLock::new(HashMap::new())),
            required_scopes: Arc::new(RwLock::new(HashMap::new())),
            refresh_locks: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            oauth_http_client: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests(keyring: SystemKeyring) -> Result<Self, AuthError> {
        Self::with_keyring(keyring)
    }

    #[cfg(test)]
    pub(crate) fn for_tests_with_oauth_http_client(
        keyring: SystemKeyring,
        oauth_http_client: Arc<dyn OAuthHttpClient>,
    ) -> Result<Self, AuthError> {
        let mut coordinator = Self::with_keyring(keyring)?;
        coordinator.oauth_http_client = Some(oauth_http_client);
        Ok(coordinator)
    }

    pub async fn status(&self, instance_id: InstanceId) -> AuthStatus {
        self.statuses
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap_or(AuthStatus::NotRequired)
    }

    pub async fn status_view(&self, instance_id: InstanceId, auth: &AuthConfig) -> AuthStatusView {
        AuthStatusView {
            instance_id,
            status: self.status(instance_id).await,
            flow: auth_flow(auth),
            scopes: auth.scopes().to_vec(),
            required_scope: self.required_scope(instance_id).await,
        }
    }

    pub async fn set_status(&self, instance_id: InstanceId, status: AuthStatus) {
        if status != AuthStatus::ScopeUpgradeRequired {
            self.required_scopes.write().await.remove(&instance_id);
        }
        self.statuses.write().await.insert(instance_id, status);
    }

    pub(crate) async fn mark_scope_upgrade_required(
        &self,
        instance_id: InstanceId,
        required_scope: Option<&str>,
    ) {
        let required_scope = required_scope
            .map(str::trim)
            .filter(|scope| !scope.is_empty());
        if let Some(required_scope) = required_scope {
            self.required_scopes
                .write()
                .await
                .insert(instance_id, required_scope.to_string());
        } else {
            self.required_scopes.write().await.remove(&instance_id);
        }
        self.statuses
            .write()
            .await
            .insert(instance_id, AuthStatus::ScopeUpgradeRequired);
    }

    pub(crate) async fn required_scope(&self, instance_id: InstanceId) -> Option<String> {
        self.required_scopes.read().await.get(&instance_id).cloned()
    }

    pub(crate) fn auth_required(
        &self,
        instance_id: InstanceId,
        auth: &AuthConfig,
    ) -> super::AuthRequired {
        auth_required(instance_id, auth)
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
        self.required_scopes.write().await.remove(&instance_id);
        self.refresh_locks.lock().await.remove(&instance_id);
        self.sessions.lock().await.remove(&instance_id);
    }

    pub async fn clear_statuses(&self) {
        self.statuses.write().await.clear();
        self.required_scopes.write().await.clear();
        self.refresh_locks.lock().await.clear();
        self.sessions.lock().await.clear();
    }

    pub async fn retain_statuses(&self, instance_ids: &HashSet<InstanceId>) {
        self.statuses
            .write()
            .await
            .retain(|instance_id, _| instance_ids.contains(instance_id));
        self.required_scopes
            .write()
            .await
            .retain(|instance_id, _| instance_ids.contains(instance_id));
        self.refresh_locks
            .lock()
            .await
            .retain(|instance_id, _| instance_ids.contains(instance_id));
        self.sessions
            .lock()
            .await
            .retain(|instance_id, _| instance_ids.contains(instance_id));
    }

    pub fn credential_store(&self, key: &AuthCredentialKey) -> KeyringCredentialStore {
        KeyringCredentialStore::with_keyring(key, self.keyring.clone())
    }

    pub fn client_secret_store(&self, key: &AuthCredentialKey) -> KeyringClientSecretStore {
        KeyringClientSecretStore::with_keyring(key, self.keyring.clone())
    }

    pub fn private_key_store(&self, key: &AuthCredentialKey) -> KeyringPrivateKeyStore {
        KeyringPrivateKeyStore::with_keyring(key, self.keyring.clone())
    }

    pub fn state_store(&self, key: &AuthCredentialKey, ttl: Duration) -> KeyringStateStore {
        KeyringStateStore::with_keyring_and_ttl(key, self.keyring.clone(), ttl)
    }

    pub async fn save_client_secret(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
        secret: ClientSecret,
    ) -> Result<(), AuthError> {
        let key = credential_key(instance_id, base_url, auth)?;
        self.client_secret_store(&key).save(&secret).await
    }

    pub async fn save_private_key(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
        private_key: PrivateKey,
    ) -> Result<(), AuthError> {
        let key = credential_key(instance_id, base_url, auth)?;
        self.private_key_store(&key).save(&private_key).await
    }

    pub async fn begin_authorization(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<AuthorizationStart, AuthError> {
        let AuthConfig::OAuthAuthorizationCode(config) = auth else {
            return Err(AuthError::UnsupportedFlow);
        };

        self.set_status(instance_id, AuthStatus::Authorizing).await;
        let result = async {
            let key = credential_key(instance_id, base_url, auth)?;
            let mut manager = self.new_manager(base_url, &key).await?;
            let metadata = manager
                .discover_metadata()
                .await
                .map_err(|_| AuthError::AuthorizationStartFailed)?;
            manager.set_metadata(metadata);

            let scopes = config.scopes.iter().map(String::as_str).collect::<Vec<_>>();
            let session = if let Some(client_id) = config.client_id.as_deref() {
                let mut oauth_config = OAuthClientConfig::new(client_id, &config.redirect_uri)
                    .with_scopes(config.scopes.clone());
                if !matches!(
                    config.client_auth_method,
                    super::AuthorizationCodeClientAuthMethod::None
                ) {
                    let secret = self
                        .client_secret_store(&key)
                        .load()
                        .await?
                        .ok_or(AuthError::MissingClientCredential)?;
                    oauth_config = oauth_config.with_client_secret(secret.expose());
                }
                manager
                    .configure_client(oauth_config)
                    .map_err(|_| AuthError::AuthorizationStartFailed)?;
                let authorization_url = manager
                    .get_authorization_url(&scopes)
                    .await
                    .map_err(|_| AuthError::AuthorizationStartFailed)?;
                AuthorizationSession::for_scope_upgrade(
                    manager,
                    authorization_url,
                    &config.redirect_uri,
                )
            } else {
                AuthorizationSession::new(
                    manager,
                    &scopes,
                    &config.redirect_uri,
                    Some("MCPStore"),
                    None,
                )
                .await
                .map_err(|_| AuthError::AuthorizationStartFailed)?
            };

            let authorization_url = session.get_authorization_url().to_string();
            self.sessions.lock().await.insert(instance_id, session);
            Ok(AuthorizationStart {
                instance_id,
                authorization_url,
                scopes: config.scopes.clone(),
            })
        }
        .await;

        if result.is_err() {
            self.set_status(instance_id, AuthStatus::Error).await;
        }
        result
    }

    pub async fn complete_authorization(
        &self,
        instance_id: InstanceId,
        callback_url: &str,
    ) -> Result<(), AuthError> {
        let callback = AuthorizationCallback::from_redirect_url(callback_url)
            .map_err(|_| AuthError::CallbackRejected)?;
        self.complete_authorization_callback(
            instance_id,
            &callback.code,
            &callback.csrf_token,
            callback.issuer.as_deref(),
        )
        .await
    }

    pub async fn complete_authorization_callback(
        &self,
        instance_id: InstanceId,
        code: &str,
        state: &str,
        issuer: Option<&str>,
    ) -> Result<(), AuthError> {
        let session = self.sessions.lock().await.remove(&instance_id);
        let Some(session) = session else {
            self.set_status(instance_id, AuthStatus::Unauthenticated)
                .await;
            return Err(AuthError::CallbackRejected);
        };

        match session
            .handle_callback_with_issuer(code, state, issuer)
            .await
        {
            Ok(_) => {
                self.set_status(instance_id, AuthStatus::Authenticated)
                    .await;
                Ok(())
            }
            Err(_) => {
                self.set_status(instance_id, AuthStatus::Unauthenticated)
                    .await;
                Err(AuthError::CallbackRejected)
            }
        }
    }

    pub async fn refresh(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<(), AuthError> {
        self.refresh_http_authorization(instance_id, base_url, auth)
            .await
            .map(|_| ())
    }

    pub(crate) async fn refresh_http_authorization(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<AuthorizationManager, AuthError> {
        let refresh_lock = self.refresh_lock(instance_id).await;
        let _refresh_guard = refresh_lock.lock().await;
        self.set_status(instance_id, AuthStatus::Refreshing).await;
        let result: Result<AuthorizationManager, AuthError> = async {
            match auth {
                AuthConfig::OAuthAuthorizationCode(_) => {
                    let key = credential_key(instance_id, base_url, auth)?;
                    let mut manager = self.new_manager(base_url, &key).await?;
                    if !manager
                        .initialize_from_store()
                        .await
                        .map_err(|_| AuthError::RefreshFailed)?
                    {
                        Err(AuthError::RefreshFailed)
                    } else {
                        manager
                            .refresh_token()
                            .await
                            .map_err(|_| AuthError::RefreshFailed)?;
                        Ok(manager)
                    }
                }
                AuthConfig::OAuthClientCredentials(_) => {
                    self.authenticate_client_credentials(instance_id, base_url, auth)
                        .await
                }
                AuthConfig::None => Err(AuthError::UnsupportedFlow),
            }
        }
        .await;

        match result {
            Ok(manager) => {
                self.set_status(instance_id, AuthStatus::Authenticated)
                    .await;
                Ok(manager)
            }
            Err(error) => {
                self.invalidate_authorization(instance_id, base_url, auth)
                    .await;
                Err(error)
            }
        }
    }

    pub(crate) async fn invalidate_authorization(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) {
        self.clear_tokens(instance_id, base_url, auth).await.ok();
        self.sessions.lock().await.remove(&instance_id);
        self.set_status(instance_id, AuthStatus::Unauthenticated)
            .await;
    }

    pub async fn begin_scope_upgrade(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
        required_scope: &str,
    ) -> Result<AuthorizationStart, AuthError> {
        let AuthConfig::OAuthAuthorizationCode(config) = auth else {
            return Err(AuthError::UnsupportedFlow);
        };
        let key = credential_key(instance_id, base_url, auth)?;
        let mut manager = self.new_manager(base_url, &key).await?;
        if !manager
            .initialize_from_store()
            .await
            .map_err(|_| AuthError::AuthorizationStartFailed)?
        {
            return Err(AuthError::Required(auth_required(instance_id, auth)));
        }
        let authorization_url = manager
            .request_scope_upgrade(required_scope)
            .await
            .map_err(|_| AuthError::AuthorizationStartFailed)?;
        let session = AuthorizationSession::for_scope_upgrade(
            manager,
            authorization_url.clone(),
            &config.redirect_uri,
        );
        self.sessions.lock().await.insert(instance_id, session);
        self.set_status(instance_id, AuthStatus::Authorizing).await;
        let mut scopes = config.scopes.clone();
        scopes.extend(required_scope.split_whitespace().map(ToString::to_string));
        scopes.sort();
        scopes.dedup();
        Ok(AuthorizationStart {
            instance_id,
            authorization_url,
            scopes,
        })
    }

    pub async fn logout(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<(), AuthError> {
        if auth.is_none() {
            return Err(AuthError::UnsupportedFlow);
        }
        let key = credential_key(instance_id, base_url, auth)?;
        self.credential_store(&key)
            .clear()
            .await
            .map_err(|_| AuthError::ProviderFailure)?;
        self.state_store(&key, AUTHORIZATION_STATE_TTL)
            .clear()
            .await?;
        self.sessions.lock().await.remove(&instance_id);
        self.set_status(instance_id, AuthStatus::Unauthenticated)
            .await;
        Ok(())
    }

    pub(crate) async fn prepare_http_authorization(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<AuthorizationManager, AuthError> {
        let key = credential_key(instance_id, base_url, auth)?;
        let mut manager = self.new_manager(base_url, &key).await?;
        let initialized = manager
            .initialize_from_store()
            .await
            .map_err(|_| AuthError::ProviderFailure)?;

        if initialized {
            match manager.get_access_token().await {
                Ok(_) => {
                    self.set_status(instance_id, AuthStatus::Authenticated)
                        .await;
                    return Ok(manager);
                }
                Err(RmcpAuthError::AuthorizationRequired) => {
                    self.credential_store(&key)
                        .clear()
                        .await
                        .map_err(|_| AuthError::ProviderFailure)?;
                }
                Err(_) => return Err(AuthError::ProviderFailure),
            }
        }

        if matches!(auth, AuthConfig::OAuthClientCredentials(_)) {
            let manager = self
                .authenticate_client_credentials(instance_id, base_url, auth)
                .await?;
            self.set_status(instance_id, AuthStatus::Authenticated)
                .await;
            return Ok(manager);
        }

        self.set_status(instance_id, AuthStatus::Unauthenticated)
            .await;
        Err(AuthError::Required(auth_required(instance_id, auth)))
    }

    async fn authenticate_client_credentials(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<AuthorizationManager, AuthError> {
        let AuthConfig::OAuthClientCredentials(config) = auth else {
            return Err(AuthError::UnsupportedFlow);
        };
        let key = credential_key(instance_id, base_url, auth)?;
        let manager = self.new_manager(base_url, &key).await?;
        let client_credentials = match config.client_auth_method {
            ClientCredentialsAuthMethod::ClientSecretPost => {
                let secret = self
                    .client_secret_store(&key)
                    .load()
                    .await?
                    .ok_or(AuthError::MissingClientCredential)?;
                ClientCredentialsConfig::ClientSecret {
                    client_id: config.client_id.clone(),
                    client_secret: secret.expose().to_string(),
                    scopes: config.scopes.clone(),
                    resource: Some(
                        config
                            .resource
                            .clone()
                            .unwrap_or_else(|| base_url.to_string()),
                    ),
                }
            }
            ClientCredentialsAuthMethod::PrivateKeyJwt => {
                let private_key = self
                    .private_key_store(&key)
                    .load()
                    .await?
                    .ok_or(AuthError::MissingClientCredential)?;
                ClientCredentialsConfig::PrivateKeyJwt {
                    client_id: config.client_id.clone(),
                    signing_key: private_key.expose().to_vec(),
                    signing_algorithm: map_signing_algorithm(&config.jwt_signing_algorithm),
                    token_endpoint_audience: config.audience.clone(),
                    scopes: config.scopes.clone(),
                    resource: Some(
                        config
                            .resource
                            .clone()
                            .unwrap_or_else(|| base_url.to_string()),
                    ),
                }
            }
        };
        let required = auth_required(instance_id, auth);
        // rmcp 2.2's private-key JWT client-credentials future is not Send across
        // the MCPStore multi-thread runtime boundary. Run the official rmcp flow
        // on its own current-thread runtime instead of reimplementing JWT exchange.
        tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| AuthError::ProviderFailure)?;
            runtime.block_on(async move {
                let mut state = OAuthState::Unauthorized(manager);
                state
                    .authenticate_client_credentials(client_credentials)
                    .await
                    .map_err(|error| match error {
                        RmcpAuthError::AuthorizationRequired => AuthError::Required(required),
                        _ => AuthError::ProviderFailure,
                    })?;
                state
                    .into_authorization_manager()
                    .ok_or(AuthError::ProviderFailure)
            })
        })
        .await
        .map_err(|_| AuthError::ProviderFailure)?
    }

    async fn refresh_lock(&self, instance_id: InstanceId) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.lock().await;
        Arc::clone(
            locks
                .entry(instance_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    async fn new_manager(
        &self,
        base_url: &str,
        key: &AuthCredentialKey,
    ) -> Result<AuthorizationManager, AuthError> {
        #[cfg(test)]
        let manager = match self.oauth_http_client.as_ref() {
            Some(client) => {
                AuthorizationManager::new_with_oauth_http_client(base_url, Arc::clone(client)).await
            }
            None => AuthorizationManager::new(base_url).await,
        };
        #[cfg(not(test))]
        let manager = AuthorizationManager::new(base_url).await;
        let mut manager = manager.map_err(|_| AuthError::ProviderFailure)?;
        manager.set_credential_store(self.credential_store(key));
        manager.set_state_store(self.state_store(key, AUTHORIZATION_STATE_TTL));
        Ok(manager)
    }

    async fn clear_tokens(
        &self,
        instance_id: InstanceId,
        base_url: &str,
        auth: &AuthConfig,
    ) -> Result<(), AuthError> {
        let key = credential_key(instance_id, base_url, auth)?;
        self.credential_store(&key)
            .clear()
            .await
            .map_err(|_| AuthError::ProviderFailure)
    }
}

fn credential_key(
    instance_id: InstanceId,
    base_url: &str,
    auth: &AuthConfig,
) -> Result<AuthCredentialKey, AuthError> {
    let client_id = match auth {
        AuthConfig::None => return Err(AuthError::UnsupportedFlow),
        AuthConfig::OAuthAuthorizationCode(config) => config
            .client_id
            .clone()
            .unwrap_or_else(|| DYNAMIC_CLIENT_IDENTITY.to_string()),
        AuthConfig::OAuthClientCredentials(config) => config.client_id.clone(),
    };
    Ok(AuthCredentialKey::new(
        instance_id,
        Some(auth.resource().unwrap_or(base_url).to_string()),
        auth.audience().map(ToString::to_string),
        client_id,
        auth.scopes().iter().cloned(),
        auth.credential_profile().map(ToString::to_string),
    ))
}

fn auth_flow(auth: &AuthConfig) -> Option<AuthFlow> {
    match auth {
        AuthConfig::None => None,
        AuthConfig::OAuthAuthorizationCode(_) => Some(AuthFlow::AuthorizationCode),
        AuthConfig::OAuthClientCredentials(_) => Some(AuthFlow::ClientCredentials),
    }
}

fn auth_required(instance_id: InstanceId, auth: &AuthConfig) -> super::AuthRequired {
    super::AuthRequired {
        instance_id,
        flow: auth_flow(auth).expect("OAuth auth config must have a flow"),
        scopes: auth.scopes().to_vec(),
    }
}

fn map_signing_algorithm(algorithm: &JwtSigningAlgorithm) -> RmcpJwtSigningAlgorithm {
    match algorithm {
        JwtSigningAlgorithm::Rs256 => RmcpJwtSigningAlgorithm::RS256,
        JwtSigningAlgorithm::Rs384 => RmcpJwtSigningAlgorithm::RS384,
        JwtSigningAlgorithm::Rs512 => RmcpJwtSigningAlgorithm::RS512,
        JwtSigningAlgorithm::Es256 => RmcpJwtSigningAlgorithm::ES256,
        JwtSigningAlgorithm::Es384 => RmcpJwtSigningAlgorithm::ES384,
    }
}
