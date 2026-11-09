use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use keyring::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use keyring::{Error as KeyringError, Result as KeyringResult};

use super::SystemKeyring;

#[derive(Default)]
struct TestSecrets {
    values: Mutex<HashMap<(String, String), Vec<u8>>>,
}

struct TestCredentialBuilder {
    secrets: Arc<TestSecrets>,
}

impl CredentialBuilderApi for TestCredentialBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> KeyringResult<Box<Credential>> {
        Ok(Box::new(TestCredential {
            secrets: Arc::clone(&self.secrets),
            key: (service.to_string(), user.to_string()),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }
}

struct TestCredential {
    secrets: Arc<TestSecrets>,
    key: (String, String),
}

impl CredentialApi for TestCredential {
    fn set_secret(&self, secret: &[u8]) -> KeyringResult<()> {
        self.secrets
            .values
            .lock()
            .unwrap()
            .insert(self.key.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(&self) -> KeyringResult<Vec<u8>> {
        self.secrets
            .values
            .lock()
            .unwrap()
            .get(&self.key)
            .cloned()
            .ok_or(KeyringError::NoEntry)
    }

    fn delete_credential(&self) -> KeyringResult<()> {
        self.secrets
            .values
            .lock()
            .unwrap()
            .remove(&self.key)
            .map(|_| ())
            .ok_or(KeyringError::NoEntry)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn test_keyring() -> SystemKeyring {
    let builder: Box<CredentialBuilder> = Box::new(TestCredentialBuilder {
        secrets: Arc::new(TestSecrets::default()),
    });
    SystemKeyring::for_tests(builder).unwrap()
}
