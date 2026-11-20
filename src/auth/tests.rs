use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::StatusCode;
use rmcp::transport::auth::{
    CredentialStore, StateStore, StoredAuthorizationState, StoredCredentials,
};

use super::*;
use crate::identity::{ScopeRef, ServiceInstanceKey};
use crate::state::{AuthState, DesiredState, ServiceState};

use super::coordinator::test_state_manager;

use super::test_support::test_keyring;

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
            "client_auth_method": "none"
        }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "client_metadata_url": "https://client.example/mcpstore.json",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback"
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
fn client_metadata_authorization_cannot_force_token_endpoint_authentication() {
    let config = serde_json::json!({
        "type": "oauth_authorization_code",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "client_metadata_url": "https://client.example/mcpstore.json",
        "client_auth_method": "client_secret_post"
    });

    let error = serde_json::from_value::<AuthConfig>(config).unwrap_err();
    assert!(error
        .to_string()
        .contains("client_auth_method requires a pre-registered auth.client_id"));
}

#[test]
fn auth_config_rejects_incomplete_or_empty_declarations() {
    let invalid = [
        serde_json::json!({
            "type": "oauth_authorization_code",
            "dynamic_client_registration": true,
            "scopes": [""]
        }),
        serde_json::json!({
            "type": "oauth_authorization_code"
        }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "client_id": "client-1",
            "client_metadata_url": "https://client.example/mcpstore.json"
        }),
        serde_json::json!({
            "type": "oauth_authorization_code",
            "client_metadata_url": "http://client.example/mcpstore.json"
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
}

#[test]
fn authorization_code_defaults_to_local_callback_with_client_metadata() {
    let config: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_metadata_url": "https://client.example/mcpstore.json",
        "redirect_uri": " "
    }))
    .unwrap();

    let AuthConfig::OAuthAuthorizationCode(config) = config else {
        panic!("expected authorization code config");
    };
    assert_eq!(config.redirect_uri, DEFAULT_OAUTH_REDIRECT_URI);
    assert_eq!(
        config.client_metadata_url.as_deref(),
        Some("https://client.example/mcpstore.json")
    );
}

#[test]
fn auth_status_serializes_as_snake_case() {
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

fn authorization_code_config() -> AuthConfig {
    serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_id": "client-1",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "scopes": ["tools.read"]
    }))
    .unwrap()
}

fn client_credentials_config() -> AuthConfig {
    serde_json::from_value(serde_json::json!({
        "type": "oauth_client_credentials",
        "client_id": "machine-client",
        "scopes": ["tools.call"],
        "client_auth_method": "client_secret_post"
    }))
    .unwrap()
}

#[test]
fn client_credentials_jwt_algorithm_is_typed_and_defaults_to_rs256() {
    let default_config = client_credentials_config();
    let AuthConfig::OAuthClientCredentials(default_config) = default_config else {
        panic!("expected client credentials config");
    };
    assert_eq!(
        default_config.jwt_signing_algorithm,
        JwtSigningAlgorithm::Rs256
    );

    for algorithm in ["rs256", "rs384", "rs512", "es256", "es384"] {
        let config: AuthConfig = serde_json::from_value(serde_json::json!({
            "type": "oauth_client_credentials",
            "client_id": "machine-client",
            "client_auth_method": "private_key_jwt",
            "jwt_signing_algorithm": algorithm
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(config).unwrap()["jwt_signing_algorithm"],
            algorithm
        );
    }

    assert!(serde_json::from_value::<AuthConfig>(serde_json::json!({
        "type": "oauth_client_credentials",
        "client_id": "machine-client",
        "client_auth_method": "private_key_jwt",
        "jwt_signing_algorithm": "hs256"
    }))
    .is_err());
}

#[tokio::test]
async fn private_key_is_persistent_isolated_and_redacted_from_debug() {
    let keyring = test_keyring();
    let alice = credential_key("alpha", &["tools.call"], Some("alice"));
    let bob = credential_key("alpha", &["tools.call"], Some("bob"));
    let store = KeyringPrivateKeyStore::with_keyring(&alice, keyring.clone());
    let private_key = PrivateKey::new(b"private-key-value".to_vec());

    store.save(&private_key).await.unwrap();
    let reopened = KeyringPrivateKeyStore::with_keyring(&alice, keyring.clone());
    assert_eq!(
        reopened.load().await.unwrap().unwrap().expose(),
        b"private-key-value"
    );
    assert_eq!(format!("{private_key:?}"), "PrivateKey([REDACTED])");
    assert!(!format!("{reopened:?}").contains("private-key-value"));

    let isolated = KeyringPrivateKeyStore::with_keyring(&bob, keyring);
    assert!(isolated.load().await.unwrap().is_none());
}

#[tokio::test]
async fn insufficient_scope_status_preserves_required_scope_until_lifecycle_changes() {
    let instance_id = instance_id("scope-upgrade");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests(test_keyring(), state_manager.clone()).unwrap();
    let auth = authorization_code_config();

    coordinator
        .mark_scope_upgrade_required(instance_id, Some("resources.read tools.call"))
        .await;
    assert_eq!(
        coordinator.status_view(instance_id, &auth).await,
        AuthStatusView {
            instance_id,
            status: AuthStatus::ScopeUpgradeRequired,
            flow: Some(AuthFlow::AuthorizationCode),
            scopes: vec!["tools.read".to_string()],
            required_scope: Some("resources.read tools.call".to_string()),
        }
    );

    coordinator
        .set_status(instance_id, AuthStatus::Authenticated)
        .await;
    assert_eq!(coordinator.required_scope(instance_id).await, None);
}

#[tokio::test]
async fn insufficient_scope_without_scope_still_requires_reauthorization() {
    let instance_id = instance_id("scope-upgrade-without-hint");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests(test_keyring(), state_manager.clone()).unwrap();

    coordinator
        .mark_scope_upgrade_required(instance_id, None)
        .await;

    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::ScopeUpgradeRequired
    );
    assert_eq!(coordinator.required_scope(instance_id).await, None);
}

#[tokio::test]
async fn authorization_code_without_credentials_returns_structured_auth_required() {
    let instance_id = instance_id("protected");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests(test_keyring(), state_manager.clone()).unwrap();
    let auth = authorization_code_config();

    let error = match coordinator
        .prepare_http_authorization(instance_id, "http://127.0.0.1:9/mcp", &auth)
        .await
    {
        Ok(_) => panic!("authorization unexpectedly succeeded"),
        Err(error) => error,
    };
    let AuthError::Required(required) = error else {
        panic!("expected structured auth required");
    };
    assert_eq!(required.instance_id, instance_id);
    assert_eq!(required.flow, AuthFlow::AuthorizationCode);
    assert_eq!(required.scopes, vec!["tools.read"]);
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Unauthenticated
    );
}

#[tokio::test]
async fn omitted_resource_uses_service_url_for_secure_credential_isolation() {
    let keyring = test_keyring();
    let instance_id = instance_id("machine");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests(keyring.clone(), state_manager.clone()).unwrap();
    let auth = client_credentials_config();
    let base_url = "https://mcp.example/mcp";

    coordinator
        .save_client_secret(
            instance_id,
            base_url,
            &auth,
            ClientSecret::new("machine-secret"),
        )
        .await
        .unwrap();

    let expected_key = AuthCredentialKey::new(
        instance_id,
        Some(base_url.to_string()),
        None,
        "machine-client",
        ["tools.call".to_string()],
        None,
    );
    let different_resource_key = AuthCredentialKey::new(
        instance_id,
        Some("https://other.example/mcp".to_string()),
        None,
        "machine-client",
        ["tools.call".to_string()],
        None,
    );

    assert_eq!(
        KeyringClientSecretStore::with_keyring(&expected_key, keyring.clone())
            .load()
            .await
            .unwrap()
            .unwrap()
            .expose(),
        "machine-secret"
    );
    assert!(
        KeyringClientSecretStore::with_keyring(&different_resource_key, keyring)
            .load()
            .await
            .unwrap()
            .is_none()
    );
}

#[derive(Default)]
struct CimdOAuthClient {
    requests: Mutex<Vec<String>>,
    supports_cimd: bool,
}

impl rmcp::transport::auth::OAuthHttpClient for CimdOAuthClient {
    fn execute(
        &self,
        request: rmcp::transport::auth::OAuthHttpRequest,
    ) -> rmcp::transport::auth::OAuthHttpClientFuture<'_> {
        let uri = request.request.uri().to_string();
        self.requests.lock().unwrap().push(uri.clone());
        let response = if uri == "https://mcp.example/mcp" {
            http::Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(
                    http::header::WWW_AUTHENTICATE,
                    r#"Bearer resource_metadata="https://mcp.example/.well-known/oauth-protected-resource""#,
                )
                .body(Vec::new())
                .unwrap()
        } else if uri == "https://mcp.example/.well-known/oauth-protected-resource" {
            http::Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(
                    serde_json::to_vec(&serde_json::json!({
                        "resource": "https://mcp.example/mcp",
                        "authorization_servers": ["https://issuer.example"]
                    }))
                    .unwrap(),
                )
                .unwrap()
        } else if uri == "https://issuer.example/.well-known/oauth-authorization-server" {
            http::Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(
                    serde_json::to_vec(&serde_json::json!({
                        "issuer": "https://issuer.example",
                        "authorization_endpoint": "https://issuer.example/authorize",
                        "token_endpoint": "https://issuer.example/token",
                        "registration_endpoint": "https://issuer.example/register",
                        "response_types_supported": ["code"],
                        "code_challenge_methods_supported": ["S256"],
                        "client_id_metadata_document_supported": self.supports_cimd
                    }))
                    .unwrap(),
                )
                .unwrap()
        } else {
            http::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap()
        };
        Box::pin(async move { Ok(response) })
    }
}

#[tokio::test]
async fn authorization_code_uses_client_metadata_url_as_client_id() {
    let oauth_client = Arc::new(CimdOAuthClient {
        supports_cimd: true,
        ..Default::default()
    });
    let instance_id = instance_id("cimd");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests_with_oauth_http_client(
        test_keyring(),
        oauth_client.clone(),
        state_manager,
    )
    .unwrap();
    let auth: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_metadata_url": "https://client.example/mcpstore.json"
    }))
    .unwrap();

    let start = coordinator
        .begin_authorization(instance_id, "https://mcp.example/mcp", &auth)
        .await
        .unwrap();
    let query = url::Url::parse(&start.authorization_url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("https://client.example/mcpstore.json")
    );
    assert!(oauth_client
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|uri| !uri.ends_with("/register")));
}

#[tokio::test]
async fn authorization_code_never_falls_back_to_dynamic_registration() {
    let oauth_client = Arc::new(CimdOAuthClient::default());
    let instance_id = instance_id("cimd-only");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests_with_oauth_http_client(
        test_keyring(),
        oauth_client.clone(),
        state_manager,
    )
    .unwrap();
    let auth: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_metadata_url": "https://client.example/mcpstore.json"
    }))
    .unwrap();

    let error = coordinator
        .begin_authorization(instance_id, "https://mcp.example/mcp", &auth)
        .await
        .unwrap_err();
    assert!(matches!(error, AuthError::InvalidConfig(_)));
    assert!(oauth_client
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|uri| !uri.ends_with("/register")));
}

const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCJRLsJP9477ViY
DNLZsxZImyGh8axjlvwhhNPuEfnQotshElqMYg3yVJUK01vwP3HAb43rfPNBi63M
7zj8yvjF9OVaowmrVWvh1jY2PATToN2o8fjvE8DnXAiUrwTwLnj+7TA39BQB2z1r
2BXNl2jL9Xdy25seOOu6xKtkRyRJ6GNtUuFC1JRTlnb9maHJd5XOY2k2DqgtD0zg
9Yy9Sf8cFJIK2n54K4Iry3oqm23NVB7E/PIZmpg2O12XxIlYcz6lWAm/FPKDqD2R
96W0NH34lVtq9HGJyl0huWeWKFryxuDGGl2Xg0Bn3tyOtS3b5/hEn60UVaTsJbnQ
uU0Es1fnAgMBAAECggEAFHhXQLKxZD6+8p8dmxENq9J/36O9FqA6RZCxKV8CjIjv
ZE1ViyVyOVGWD6Zjnv5ZwHNCTDxMFa8L7N7odiuZ5qy1voryqFtq5PjqCQjGmtKc
oOhDZvpgpCqBgxQIiKgS1h1ZOUw2PEKitGA7JszDTYBztCv97rIEAUxOgiaTyaDc
sWLGxsbt+x3P4+egHn86Y296FR/E+rrRdZU01W5V939/eTBsMqiahICPtIMmuo0F
NV8o+Mv6WHapgzj2nwOi+K8rgYPqZdHlIh96yiP1voLj+otnj5pyOegCYp3Bu80+
FDd/JGogYVo2ZXLGnxV1kKSLtaHZgz03aQsJyvam+QKBgQDBfnOCdRAV07YZyGMs
VkUa4Z2JkiHEy4B0x0qhdAZv4yrDLfkUxhF/locSfkq4MsbdDwzkytBkgKnkBXPz
c1ZTw3bLJHJY6pU7JAY5BcUY/wl25kithty9TQtzKgASGJ281zHndQqQEzTC0Cx8
MgFcCVMocH719SeNJbgI5nLQqQKBgQC1nIynU0c49Qfpqf/Aazq7/cWp38hsV3li
25VH0EFnze/vEodmSdCQ1QWRhmy/kLfUGYJyUoYaJk7KHgWjrZVS4dvTe4S+yre3
HctvwYzjQfTrHvHQmhX7HfFbm1VuB2cs94heEsGyg511sLmhsXpBEsB1F+zdulI4
DAMVB5zuDwKBgEkli3cq19zYfwO6LDuLlW43Ej36f0eNAs+is0Tbvr83amgEjh/b
TKwl9IP6ODbwAxt4YBBx11vXA+KOaSoEVQMvZk4fRhb0/1svICcYVk0/xI0tOxZW
YEYzxPtRSluM8Lx9wYDVTxvuFsj6t4ZvxPHNGKG1/Vjvx3blZm/+5jKZAoGAFGX2
AmE/Ma2L6vnWKQWiPjU9u1vQRiL5Flp1hPBmOEOQPHkHTjziOTJEAtlnY4jcrO0E
ktSkDVHaLad7mKvJhtqpdzJ7cXaRdfbZv76slWX2HWaHYJe9+kudrV1gFhCszQcs
gOx4ZxWTXQGxh/DIO4DgrwY1652e2H645ebKAI8CgYEAuS1uLivMnMq/qZ7tZZcH
1wNScMgC//FnLdDCym798kmcdV206cwFpNDjGVIoAzu6NLCP1ZXoIAV3tpy5xutX
4aJhudwgENuY3/dA/Ctg/z+gdrTORZArJk/wPbaDZl50z2IAAZOKIsE4j2dMjhuv
KQ5gIE4DfZfX//DZbSy2UiQ=
-----END PRIVATE KEY-----"#;

#[derive(Default)]
struct PrivateKeyJwtOAuthClient {
    token_form: Mutex<Option<HashMap<String, String>>>,
}

impl rmcp::transport::auth::OAuthHttpClient for PrivateKeyJwtOAuthClient {
    fn execute(
        &self,
        request: rmcp::transport::auth::OAuthHttpRequest,
    ) -> rmcp::transport::auth::OAuthHttpClientFuture<'_> {
        let uri = request.request.uri().to_string();
        let response = if uri == "https://mcp.example/mcp" {
            http::Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(
                    http::header::WWW_AUTHENTICATE,
                    r#"Bearer resource_metadata="https://mcp.example/.well-known/oauth-protected-resource""#,
                )
                .body(Vec::new())
                .unwrap()
        } else if uri == "https://mcp.example/.well-known/oauth-protected-resource" {
            http::Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(
                    serde_json::to_vec(&serde_json::json!({
                        "resource": "https://mcp.example/mcp",
                        "authorization_servers": ["https://issuer.example"]
                    }))
                    .unwrap(),
                )
                .unwrap()
        } else if uri == "https://issuer.example/.well-known/oauth-authorization-server" {
            http::Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(
                    serde_json::to_vec(&serde_json::json!({
                        "issuer": "https://issuer.example",
                        "authorization_endpoint": "https://issuer.example/authorize",
                        "token_endpoint": "https://issuer.example/token",
                        "response_types_supported": ["code"],
                        "token_endpoint_auth_methods_supported": ["private_key_jwt"],
                        "token_endpoint_auth_signing_alg_values_supported": ["RS256"]
                    }))
                    .unwrap(),
                )
                .unwrap()
        } else if uri == "https://issuer.example/token" {
            let request_url = reqwest::Url::parse(&format!(
                "https://fixture.invalid/?{}",
                String::from_utf8_lossy(request.request.body())
            ))
            .unwrap();
            let form = request_url
                .query_pairs()
                .into_owned()
                .collect::<HashMap<_, _>>();
            *self.token_form.lock().unwrap() = Some(form);
            http::Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(
                    serde_json::to_vec(&serde_json::json!({
                        "access_token": "private-jwt-access-token",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "scope": "tools.call"
                    }))
                    .unwrap(),
                )
                .unwrap()
        } else {
            http::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap()
        };
        Box::pin(async move { Ok(response) })
    }
}

#[tokio::test]
async fn private_key_jwt_uses_rmcp_official_client_credentials_flow() {
    use base64::Engine;

    let oauth_client = Arc::new(PrivateKeyJwtOAuthClient::default());
    let instance_id = instance_id("private-key-jwt");
    let state_manager = test_state_manager();
    state_manager
        .create(ServiceState::new(
            instance_id,
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Stopped,
            AuthState::NotRequired,
            0,
        ))
        .await
        .unwrap();
    let coordinator = AuthCoordinator::for_tests_with_oauth_http_client(
        test_keyring(),
        oauth_client.clone(),
        state_manager.clone(),
    )
    .unwrap();
    let mcp_url = "https://mcp.example/mcp";
    let auth: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_client_credentials",
        "client_id": "machine-client",
        "scopes": ["tools.call"],
        "client_auth_method": "private_key_jwt",
        "jwt_signing_algorithm": "rs256"
    }))
    .unwrap();

    coordinator
        .save_private_key(
            instance_id,
            mcp_url,
            &auth,
            PrivateKey::new(TEST_RSA_PRIVATE_KEY.as_bytes().to_vec()),
        )
        .await
        .unwrap();
    let manager = coordinator
        .prepare_http_authorization(instance_id, mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "private-jwt-access-token"
    );

    let form = oauth_client.token_form.lock().unwrap().clone().unwrap();
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some("client_credentials")
    );
    assert_eq!(
        form.get("client_assertion_type").map(String::as_str),
        Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer")
    );
    assert_eq!(form.get("resource").map(String::as_str), Some(mcp_url));
    assert!(!form.contains_key("client_id"));

    let assertion = form.get("client_assertion").unwrap();
    let segments = assertion.split('.').collect::<Vec<_>>();
    assert_eq!(segments.len(), 3);
    let header: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(segments[0])
            .unwrap(),
    )
    .unwrap();
    let claims: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(segments[1])
            .unwrap(),
    )
    .unwrap();
    assert_eq!(header["alg"], "RS256");
    assert_eq!(claims["sub"], "machine-client");
    assert_eq!(claims["aud"], "https://issuer.example");
}
