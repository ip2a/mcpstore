use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ::keyring::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use ::keyring::{Error as KeyringError, Result as KeyringResult};
use rmcp::transport::auth::{
    CredentialStore, StateStore, StoredAuthorizationState, StoredCredentials,
};

use super::*;
use crate::identity::{ScopeRef, ServiceInstanceKey};

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

fn test_keyring() -> SystemKeyring {
    let builder: Box<CredentialBuilder> = Box::new(TestCredentialBuilder {
        secrets: Arc::new(TestSecrets::default()),
    });
    SystemKeyring::for_tests(builder).unwrap()
}

fn instance_id(name: &str) -> crate::identity::InstanceId {
    ServiceInstanceKey::new(name, ScopeRef::Store).instance_id()
}

fn credential_key(service_name: &str, scopes: &[&str], profile: Option<&str>) -> AuthCredentialKey {
    AuthCredentialKey::new(
        instance_id(service_name),
        Some("https://mcp.example/resource".to_string()),
        Some("https://api.example".to_string()),
        "client-1",
        scopes.iter().map(|scope| (*scope).to_string()),
        profile.map(str::to_string),
    )
}

fn stored_credentials() -> StoredCredentials {
    serde_json::from_value(serde_json::json!({
        "client_id": "client-1",
        "token_response": {
            "access_token": "access-secret-value",
            "token_type": "bearer",
            "expires_in": 3600,
            "refresh_token": "refresh-secret-value",
            "scope": "tools.read tools.call"
        },
        "granted_scopes": ["tools.read", "tools.call"],
        "token_received_at": 1_700_000_000
    }))
    .unwrap()
}

fn authorization_state(created_at: u64) -> StoredAuthorizationState {
    serde_json::from_value(serde_json::json!({
        "pkce_verifier": "pkce-secret-value",
        "csrf_token": "csrf-secret-value",
        "expected_issuer": "https://issuer.example",
        "require_issuer": true,
        "created_at": created_at
    }))
    .unwrap()
}

#[test]
fn auth_config_supports_all_declared_flows_without_secret_fields() {
    let values = [
        serde_json::json!({ "type": "none" }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "client_id": "client-1",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
            "scopes": ["tools.read"],
            "resource": "https://mcp.example/resource",
            "audience": "https://api.example",
            "credential_profile": "alice",
            "dynamic_client_registration": false,
            "client_auth_method": "none"
        }),
        serde_json::json!({
            "type": "oauth_client_credentials",
            "client_id": "machine-client",
            "scopes": ["tools.call"],
            "resource": "https://mcp.example/resource",
            "client_auth_method": "client_secret_post"
        }),
    ];

    for value in values {
        let config: AuthConfig = serde_json::from_value(value).unwrap();
        let serialized = serde_json::to_value(&config).unwrap();
        let object = serialized.as_object().unwrap();
        for secret_key in [
            "access_token",
            "refresh_token",
            "client_secret",
            "pkce_verifier",
            "csrf_token",
        ] {
            assert!(!object.contains_key(secret_key));
        }
    }
}

#[test]
fn auth_config_rejects_secret_fields() {
    for secret_field in [
        "access_token",
        "refresh_token",
        "client_secret",
        "pkce_verifier",
        "oauth_state",
    ] {
        let mut value = serde_json::json!({
            "type": "oauth_authorization_code",
            "client_id": "client-1",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback"
        });
        value.as_object_mut().unwrap().insert(
            secret_field.to_string(),
            serde_json::json!("must-not-be-accepted"),
        );
        assert!(serde_json::from_value::<AuthConfig>(value).is_err());
    }
}

#[test]
fn auth_config_rejects_incomplete_or_empty_declarations() {
    let invalid = [
        serde_json::json!({
            "type": "oauth_authorization_code",
            "client_id": "client-1",
            "redirect_uri": " "
        }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback"
        }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
            "dynamic_client_registration": true,
            "scopes": [""]
        }),
        serde_json::json!({
            "type": "oauth_client_credentials",
            "client_id": " "
        }),
        serde_json::json!({
            "type": "oauth_client_credentials",
            "client_id": "machine-client",
            "resource": ""
        }),
    ];

    for value in invalid {
        assert!(serde_json::from_value::<AuthConfig>(value).is_err());
    }

    let dynamic: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "dynamic_client_registration": true
    }))
    .unwrap();
    assert!(matches!(dynamic, AuthConfig::OAuthAuthorizationCode(_)));
}

#[test]
fn auth_status_serialization_is_independent_from_connection_status() {
    let statuses = [
        (AuthStatus::NotRequired, "not_required"),
        (AuthStatus::Unauthenticated, "unauthenticated"),
        (AuthStatus::Authorizing, "authorizing"),
        (AuthStatus::Authenticated, "authenticated"),
        (AuthStatus::Refreshing, "refreshing"),
        (AuthStatus::Error, "error"),
    ];

    for (status, expected) in statuses {
        assert_eq!(serde_json::to_value(status).unwrap(), expected);
    }
}

#[test]
fn credential_identity_normalizes_scopes_and_isolates_security_domains() {
    let base = credential_key("alpha", &["tools.call", "tools.read"], Some("alice"));
    let reordered = credential_key("alpha", &["tools.read", "tools.call"], Some("alice"));
    let other_scope = credential_key("alpha", &["tools.read"], Some("alice"));
    let other_service = credential_key("beta", &["tools.call", "tools.read"], Some("alice"));
    let other_profile = credential_key("alpha", &["tools.call", "tools.read"], Some("bob"));
    let other_resource = AuthCredentialKey::new(
        instance_id("alpha"),
        Some("https://other.example/resource".to_string()),
        Some("https://api.example".to_string()),
        "client-1",
        ["tools.call".to_string(), "tools.read".to_string()],
        Some("alice".to_string()),
    );
    let other_client = AuthCredentialKey::new(
        instance_id("alpha"),
        Some("https://mcp.example/resource".to_string()),
        Some("https://api.example".to_string()),
        "client-2",
        ["tools.call".to_string(), "tools.read".to_string()],
        Some("alice".to_string()),
    );

    assert_eq!(base.storage_id(), reordered.storage_id());
    assert_eq!(base.scope_hash(), reordered.scope_hash());
    for distinct in [
        other_scope,
        other_service,
        other_profile,
        other_resource,
        other_client,
    ] {
        assert_ne!(base.storage_id(), distinct.storage_id());
    }
}

#[tokio::test]
async fn credentials_survive_store_recreation_without_cross_scope_leakage() {
    let keyring = test_keyring();
    let full_scope = credential_key("alpha", &["tools.read", "tools.call"], Some("alice"));
    let read_only = credential_key("alpha", &["tools.read"], Some("alice"));

    let first = KeyringCredentialStore::with_keyring(&full_scope, keyring.clone());
    first.save(stored_credentials()).await.unwrap();
    drop(first);

    let reopened = KeyringCredentialStore::with_keyring(&full_scope, keyring.clone());
    let loaded = reopened.load().await.unwrap().unwrap();
    let loaded_json = serde_json::to_value(loaded).unwrap().to_string();
    assert!(loaded_json.contains("access-secret-value"));
    assert!(loaded_json.contains("refresh-secret-value"));

    let isolated = KeyringCredentialStore::with_keyring(&read_only, keyring);
    assert!(isolated.load().await.unwrap().is_none());
}

#[tokio::test]
async fn client_secret_is_persistent_isolated_and_redacted_from_debug() {
    let keyring = test_keyring();
    let alice = credential_key("alpha", &["tools.call"], Some("alice"));
    let bob = credential_key("alpha", &["tools.call"], Some("bob"));
    let store = KeyringClientSecretStore::with_keyring(&alice, keyring.clone());
    let secret = ClientSecret::new("client-secret-value");

    store.save(&secret).await.unwrap();
    let reopened = KeyringClientSecretStore::with_keyring(&alice, keyring.clone());
    assert_eq!(
        reopened.load().await.unwrap().unwrap().expose(),
        "client-secret-value"
    );
    assert_eq!(format!("{secret:?}"), "ClientSecret([REDACTED])");
    assert!(!format!("{reopened:?}").contains("client-secret-value"));
    assert!(!AuthError::InvalidStoredData
        .to_string()
        .contains("client-secret-value"));

    let isolated = KeyringClientSecretStore::with_keyring(&bob, keyring);
    assert!(isolated.load().await.unwrap().is_none());
}

#[tokio::test]
async fn authorization_state_is_separate_persistent_and_expirable() {
    let keyring = test_keyring();
    let key = credential_key("alpha", &["tools.call"], Some("alice"));
    let credentials = KeyringCredentialStore::with_keyring(&key, keyring.clone());
    credentials.save(stored_credentials()).await.unwrap();

    let state_store =
        KeyringStateStore::with_keyring_and_ttl(&key, keyring.clone(), Duration::from_secs(600));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state_store
        .save("current-csrf", authorization_state(now))
        .await
        .unwrap();
    assert!(state_store.load("current-csrf").await.unwrap().is_some());
    drop(state_store);

    let reopened =
        KeyringStateStore::with_keyring_and_ttl(&key, keyring.clone(), Duration::from_secs(600));
    assert!(reopened.load("current-csrf").await.unwrap().is_some());
    reopened
        .save("expired-csrf", authorization_state(now.saturating_sub(601)))
        .await
        .unwrap();
    assert!(reopened.load("expired-csrf").await.unwrap().is_none());
    assert_eq!(reopened.purge_expired().await.unwrap(), 0);

    assert!(credentials.load().await.unwrap().is_some());
    reopened.delete("current-csrf").await.unwrap();
    assert!(reopened.load("current-csrf").await.unwrap().is_none());
    assert!(credentials.load().await.unwrap().is_some());
}
