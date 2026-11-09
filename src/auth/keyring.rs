use std::sync::Arc;

use ::keyring::credential::CredentialPersistence;
use ::keyring::{CredentialBuilder, Entry, Error as KeyringError};

use super::AuthError;

#[derive(Clone)]
pub(crate) struct SystemKeyring {
    builder: Arc<CredentialBuilder>,
}

impl std::fmt::Debug for SystemKeyring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SystemKeyring")
    }
}

impl SystemKeyring {
    pub(crate) fn new() -> Result<Self, AuthError> {
        Self::from_builder(::keyring::default::default_credential_builder())
    }

    fn from_builder(builder: Box<CredentialBuilder>) -> Result<Self, AuthError> {
        if !matches!(builder.persistence(), CredentialPersistence::UntilDelete) {
            return Err(AuthError::SecureStorage {
                operation: "initialize",
                kind: "non_persistent_backend",
            });
        }
        Ok(Self {
            builder: Arc::from(builder),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests(builder: Box<CredentialBuilder>) -> Result<Self, AuthError> {
        Self::from_builder(builder)
    }

    async fn entry(&self, service: &'static str, account: String) -> Result<Entry, AuthError> {
        let builder = Arc::clone(&self.builder);
        tokio::task::spawn_blocking(move || {
            builder
                .build(None, service, &account)
                .map(Entry::new_with_credential)
                .map_err(|error| map_keyring_error("open", error))
        })
        .await
        .map_err(|_| AuthError::SecureStorage {
            operation: "open",
            kind: "worker_failure",
        })?
    }

    pub(crate) async fn load(
        &self,
        service: &'static str,
        account: String,
    ) -> Result<Option<Vec<u8>>, AuthError> {
        let entry = self.entry(service, account).await?;
        tokio::task::spawn_blocking(move || match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error("load", error)),
        })
        .await
        .map_err(|_| AuthError::SecureStorage {
            operation: "load",
            kind: "worker_failure",
        })?
    }

    pub(crate) async fn save(
        &self,
        service: &'static str,
        account: String,
        secret: Vec<u8>,
    ) -> Result<(), AuthError> {
        let entry = self.entry(service, account).await?;
        tokio::task::spawn_blocking(move || {
            entry
                .set_secret(&secret)
                .map_err(|error| map_keyring_error("save", error))
        })
        .await
        .map_err(|_| AuthError::SecureStorage {
            operation: "save",
            kind: "worker_failure",
        })?
    }

    pub(crate) async fn delete(
        &self,
        service: &'static str,
        account: String,
    ) -> Result<(), AuthError> {
        let entry = self.entry(service, account).await?;
        tokio::task::spawn_blocking(move || match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error("delete", error)),
        })
        .await
        .map_err(|_| AuthError::SecureStorage {
            operation: "delete",
            kind: "worker_failure",
        })?
    }
}

fn map_keyring_error(operation: &'static str, error: KeyringError) -> AuthError {
    let kind = match error {
        KeyringError::PlatformFailure(_) => "platform_failure",
        KeyringError::NoStorageAccess(_) => "access_denied",
        KeyringError::NoEntry => "not_found",
        KeyringError::BadEncoding(_) => "invalid_encoding",
        KeyringError::TooLong(_, _) => "identifier_too_long",
        KeyringError::Invalid(_, _) => "invalid_identifier",
        KeyringError::Ambiguous(_) => "ambiguous_entry",
        _ => "unknown",
    };
    AuthError::SecureStorage { operation, kind }
}
