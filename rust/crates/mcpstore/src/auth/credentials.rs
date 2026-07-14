use async_trait::async_trait;
use rmcp::transport::auth::{AuthError as RmcpAuthError, CredentialStore, StoredCredentials};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{AuthCredentialKey, AuthError, SystemKeyring};

const OAUTH_CREDENTIAL_SERVICE: &str = "mcpstore.oauth.credentials";
const OAUTH_CLIENT_SECRET_SERVICE: &str = "mcpstore.oauth.client-secret";

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ClientSecret(String);

impl std::fmt::Debug for ClientSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ClientSecret([REDACTED])")
    }
}

impl ClientSecret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

pub struct KeyringCredentialStore {
    keyring: SystemKeyring,
    account: String,
}

impl std::fmt::Debug for KeyringCredentialStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyringCredentialStore")
            .field("account", &self.account)
            .finish()
    }
}

impl KeyringCredentialStore {
    pub fn new(key: &AuthCredentialKey) -> Result<Self, AuthError> {
        Ok(Self::with_keyring(key, SystemKeyring::new()?))
    }

    pub(crate) fn with_keyring(key: &AuthCredentialKey, keyring: SystemKeyring) -> Self {
        Self {
            keyring,
            account: key.storage_id(),
        }
    }
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, RmcpAuthError> {
        let Some(bytes) = self
            .keyring
            .load(OAUTH_CREDENTIAL_SERVICE, self.account.clone())
            .await
            .map_err(to_rmcp_error)?
        else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes).map(Some).map_err(|_| {
            RmcpAuthError::InternalError("stored OAuth credentials are invalid".into())
        })
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), RmcpAuthError> {
        let bytes = serde_json::to_vec(&credentials).map_err(|_| {
            RmcpAuthError::InternalError("OAuth credentials could not be serialized".into())
        })?;
        self.keyring
            .save(OAUTH_CREDENTIAL_SERVICE, self.account.clone(), bytes)
            .await
            .map_err(to_rmcp_error)
    }

    async fn clear(&self) -> Result<(), RmcpAuthError> {
        self.keyring
            .delete(OAUTH_CREDENTIAL_SERVICE, self.account.clone())
            .await
            .map_err(to_rmcp_error)
    }
}

pub struct KeyringClientSecretStore {
    keyring: SystemKeyring,
    account: String,
}

impl std::fmt::Debug for KeyringClientSecretStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyringClientSecretStore")
            .field("account", &self.account)
            .finish()
    }
}

impl KeyringClientSecretStore {
    pub fn new(key: &AuthCredentialKey) -> Result<Self, AuthError> {
        Ok(Self::with_keyring(key, SystemKeyring::new()?))
    }

    pub(crate) fn with_keyring(key: &AuthCredentialKey, keyring: SystemKeyring) -> Self {
        Self {
            keyring,
            account: key.storage_id(),
        }
    }

    pub async fn load(&self) -> Result<Option<ClientSecret>, AuthError> {
        let secret = self
            .keyring
            .load(OAUTH_CLIENT_SECRET_SERVICE, self.account.clone())
            .await?;
        secret
            .map(String::from_utf8)
            .transpose()
            .map(|value| value.map(ClientSecret::new))
            .map_err(|_| AuthError::InvalidStoredData)
    }

    pub async fn save(&self, secret: &ClientSecret) -> Result<(), AuthError> {
        self.keyring
            .save(
                OAUTH_CLIENT_SECRET_SERVICE,
                self.account.clone(),
                secret.expose().as_bytes().to_vec(),
            )
            .await
    }

    pub async fn clear(&self) -> Result<(), AuthError> {
        self.keyring
            .delete(OAUTH_CLIENT_SECRET_SERVICE, self.account.clone())
            .await
    }
}

fn to_rmcp_error(error: AuthError) -> RmcpAuthError {
    RmcpAuthError::InternalError(error.to_string())
}
