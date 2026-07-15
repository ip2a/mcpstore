use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use axum::{
    extract::{Form, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use rmcp::transport::auth::{
    CredentialStore, StateStore, StoredAuthorizationState, StoredCredentials,
};

use super::*;
use crate::identity::{ScopeRef, ServiceInstanceKey};

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
fn dynamic_authorization_cannot_force_token_endpoint_authentication() {
    let config = serde_json::json!({
        "type": "oauth_authorization_code",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "dynamic_client_registration": true,
        "client_auth_method": "client_secret_post"
    });

    let error = serde_json::from_value::<AuthConfig>(config).unwrap_err();
    assert!(error
        .to_string()
        .contains("client_auth_method cannot be forced"));
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
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("scope-upgrade");
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
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("scope-upgrade-without-hint");

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
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("protected");
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
    let coordinator = AuthCoordinator::for_tests(keyring.clone()).unwrap();
    let instance_id = instance_id("machine");
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
        let response = if uri.ends_with("/.well-known/oauth-authorization-server/mcp") {
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
    let coordinator =
        AuthCoordinator::for_tests_with_oauth_http_client(test_keyring(), oauth_client.clone())
            .unwrap();
    let instance_id = instance_id("private-key-jwt");
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
    assert_eq!(claims["aud"], "https://issuer.example/token");
}

#[derive(Clone)]
struct OAuthFixtureState {
    issuer: String,
    token_requests: Arc<AtomicUsize>,
    refresh_requests: Arc<AtomicUsize>,
    reject_refresh: Arc<AtomicBool>,
    registration_requests: Arc<AtomicUsize>,
    last_token_form: Arc<Mutex<Option<HashMap<String, String>>>>,
}

struct OAuthFixture {
    base_url: String,
    token_requests: Arc<AtomicUsize>,
    refresh_requests: Arc<AtomicUsize>,
    reject_refresh: Arc<AtomicBool>,
    registration_requests: Arc<AtomicUsize>,
    last_token_form: Arc<Mutex<Option<HashMap<String, String>>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for OAuthFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl OAuthFixture {
    async fn spawn() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let token_requests = Arc::new(AtomicUsize::new(0));
        let refresh_requests = Arc::new(AtomicUsize::new(0));
        let reject_refresh = Arc::new(AtomicBool::new(false));
        let registration_requests = Arc::new(AtomicUsize::new(0));
        let last_token_form = Arc::new(Mutex::new(None));
        let state = OAuthFixtureState {
            issuer: base_url.clone(),
            token_requests: Arc::clone(&token_requests),
            refresh_requests: Arc::clone(&refresh_requests),
            reject_refresh: Arc::clone(&reject_refresh),
            registration_requests: Arc::clone(&registration_requests),
            last_token_form: Arc::clone(&last_token_form),
        };
        let app = Router::new()
            .route(
                "/.well-known/oauth-authorization-server/mcp",
                get(oauth_metadata),
            )
            .route("/token", post(oauth_token))
            .route("/register", post(oauth_register))
            .with_state(state);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url,
            token_requests,
            refresh_requests,
            reject_refresh,
            registration_requests,
            last_token_form,
            task,
        }
    }

    fn mcp_url(&self) -> String {
        format!("{}/mcp", self.base_url)
    }
}

async fn oauth_metadata(State(state): State<OAuthFixtureState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "issuer": state.issuer,
        "authorization_endpoint": format!("{}/authorize", state.issuer),
        "token_endpoint": format!("{}/token", state.issuer),
        "registration_endpoint": format!("{}/register", state.issuer),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none", "client_secret_post"],
        "authorization_response_iss_parameter_supported": true
    }))
}

async fn oauth_token(
    State(state): State<OAuthFixtureState>,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<serde_json::Value>) {
    state.token_requests.fetch_add(1, Ordering::SeqCst);
    *state.last_token_form.lock().unwrap() = Some(form.clone());

    let grant_type = form.get("grant_type").map(String::as_str);
    let valid = match grant_type {
        Some("authorization_code") => {
            form.get("code").map(String::as_str) == Some("valid-code")
                && form
                    .get("code_verifier")
                    .is_some_and(|value| !value.is_empty())
        }
        Some("client_credentials") => {
            form.get("client_id").map(String::as_str) == Some("machine-client")
                && form.get("client_secret").map(String::as_str) == Some("machine-secret")
        }
        Some("refresh_token") => {
            state.refresh_requests.fetch_add(1, Ordering::SeqCst);
            !state.reject_refresh.load(Ordering::SeqCst)
                && form.get("refresh_token").map(String::as_str) == Some("fixture-refresh-token")
        }
        _ => false,
    };

    if !valid {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "fixture rejected credentials"
            })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": if grant_type == Some("refresh_token") {
                "fixture-refreshed-token"
            } else {
                "fixture-access-token"
            },
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "fixture-refresh-token",
            "scope": "tools.read tools.call"
        })),
    )
}

async fn oauth_register(
    State(state): State<OAuthFixtureState>,
    Json(_request): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    state.registration_requests.fetch_add(1, Ordering::SeqCst);
    Json(serde_json::json!({
        "client_id": "dynamic-client-id",
        "redirect_uris": ["http://127.0.0.1:8787/oauth/callback"],
        "token_endpoint_auth_method": "none"
    }))
}

fn authorization_state_from_url(authorization_url: &str) -> String {
    reqwest::Url::parse(authorization_url)
        .unwrap()
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("authorization URL must contain state")
}

#[tokio::test]
async fn authorization_code_validates_state_pkce_and_restores_tokens_after_restart() {
    let fixture = OAuthFixture::spawn().await;
    let keyring = test_keyring();
    let coordinator = AuthCoordinator::for_tests(keyring.clone()).unwrap();
    let instance_id = instance_id("oauth-lifecycle");
    let auth = authorization_code_config();
    let mcp_url = fixture.mcp_url();

    let first = coordinator
        .begin_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    let first_url = reqwest::Url::parse(&first.authorization_url).unwrap();
    assert!(first_url
        .query_pairs()
        .any(|(key, value)| key == "code_challenge_method" && value == "S256"));
    assert!(first_url
        .query_pairs()
        .any(|(key, value)| key == "resource" && value == mcp_url));

    let mismatch = coordinator
        .complete_authorization_callback(
            instance_id,
            "valid-code",
            "wrong-state",
            Some(&fixture.base_url),
        )
        .await;
    assert!(matches!(mismatch, Err(AuthError::CallbackRejected)));
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 0);
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Unauthenticated
    );

    let invalid = coordinator
        .begin_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    let invalid_state = authorization_state_from_url(&invalid.authorization_url);
    let invalid_result = coordinator
        .complete_authorization_callback(
            instance_id,
            "invalid-code",
            &invalid_state,
            Some(&fixture.base_url),
        )
        .await;
    assert!(matches!(invalid_result, Err(AuthError::CallbackRejected)));
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 1);

    let valid = coordinator
        .begin_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    let valid_state = authorization_state_from_url(&valid.authorization_url);
    coordinator
        .complete_authorization_callback(
            instance_id,
            "valid-code",
            &valid_state,
            Some(&fixture.base_url),
        )
        .await
        .unwrap();
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Authenticated
    );
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
    let token_form = fixture.last_token_form.lock().unwrap().clone().unwrap();
    assert_eq!(token_form.get("resource"), Some(&mcp_url));
    assert!(token_form
        .get("code_verifier")
        .is_some_and(|value| !value.is_empty()));

    drop(coordinator);
    let restarted = AuthCoordinator::for_tests(keyring).unwrap();
    let manager = restarted
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-access-token"
    );
    assert_eq!(
        restarted.status(instance_id).await,
        AuthStatus::Authenticated
    );
}

#[tokio::test]
async fn authorization_code_rejects_resource_not_supported_by_rmcp_public_api() {
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("authorization-resource");
    let auth: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_id": "client-1",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "resource": "https://other.example/mcp"
    }))
    .unwrap();

    let error = coordinator
        .begin_authorization(instance_id, "https://mcp.example/mcp", &auth)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("auth.resource must be omitted or equal to the service URL"));
}

#[tokio::test]
async fn authorization_code_rejects_client_auth_method_rmcp_would_not_use() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("authorization-client-auth");
    let auth: AuthConfig = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_id": "client-1",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "client_auth_method": "client_secret_basic"
    }))
    .unwrap();

    let error = coordinator
        .begin_authorization(instance_id, &fixture.mcp_url(), &auth)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("cannot be forced through the public API"));
}

fn dynamic_authorization_code_config() -> AuthConfig {
    serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "scopes": ["tools.read"],
        "dynamic_client_registration": true
    }))
    .unwrap()
}

#[tokio::test]
async fn dynamic_client_registration_uses_rmcp_session_and_persists_assigned_client() {
    let fixture = OAuthFixture::spawn().await;
    let keyring = test_keyring();
    let coordinator = AuthCoordinator::for_tests(keyring.clone()).unwrap();
    let instance_id = instance_id("dynamic-oauth");
    let auth = dynamic_authorization_code_config();
    let mcp_url = fixture.mcp_url();

    let start = coordinator
        .begin_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(fixture.registration_requests.load(Ordering::SeqCst), 1);
    let authorization_url = reqwest::Url::parse(&start.authorization_url).unwrap();
    assert!(authorization_url
        .query_pairs()
        .any(|(key, value)| key == "client_id" && value == "dynamic-client-id"));
    let state = authorization_state_from_url(&start.authorization_url);

    coordinator
        .complete_authorization_callback(instance_id, "valid-code", &state, Some(&fixture.base_url))
        .await
        .unwrap();
    drop(coordinator);

    let restarted = AuthCoordinator::for_tests(keyring).unwrap();
    let manager = restarted
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-access-token"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_credentials_defaults_resource_to_mcp_url_and_authenticates_with_rmcp() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("client-credentials");
    let auth = client_credentials_config();
    let mcp_url = fixture.mcp_url();

    coordinator
        .save_client_secret(
            instance_id,
            &mcp_url,
            &auth,
            ClientSecret::new("machine-secret"),
        )
        .await
        .unwrap();
    let manager = coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();

    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-access-token"
    );
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Authenticated
    );
    let token_form = fixture.last_token_form.lock().unwrap().clone().unwrap();
    assert_eq!(
        token_form.get("grant_type").map(String::as_str),
        Some("client_credentials")
    );
    assert_eq!(token_form.get("resource"), Some(&mcp_url));
}

async fn complete_fixture_authorization(
    coordinator: &AuthCoordinator,
    fixture: &OAuthFixture,
    instance_id: crate::identity::InstanceId,
    auth: &AuthConfig,
) {
    let start = coordinator
        .begin_authorization(instance_id, &fixture.mcp_url(), auth)
        .await
        .unwrap();
    let state = authorization_state_from_url(&start.authorization_url);
    coordinator
        .complete_authorization_callback(instance_id, "valid-code", &state, Some(&fixture.base_url))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_refreshes_are_serialized_per_instance() {
    let fixture = OAuthFixture::spawn().await;
    let keyring = test_keyring();
    let coordinator = AuthCoordinator::for_tests(keyring).unwrap();
    let instance_id = instance_id("concurrent-refresh");
    let auth = authorization_code_config();
    let mcp_url = fixture.mcp_url();

    complete_fixture_authorization(&coordinator, &fixture, instance_id, &auth).await;

    let first = coordinator.refresh(instance_id, &mcp_url, &auth);
    let second = coordinator.refresh(instance_id, &mcp_url, &auth);
    let (first_result, second_result) = tokio::join!(first, second);
    assert!(first_result.is_ok());
    assert!(second_result.is_ok());
    assert_eq!(fixture.refresh_requests.load(Ordering::SeqCst), 2);
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Authenticated
    );

    let manager = coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-refreshed-token"
    );
}

#[tokio::test]
async fn refresh_failure_clears_tokens_and_requires_authorization_again() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("refresh-lifecycle");
    let auth = authorization_code_config();
    let mcp_url = fixture.mcp_url();
    complete_fixture_authorization(&coordinator, &fixture, instance_id, &auth).await;

    coordinator
        .refresh(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(fixture.refresh_requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        fixture
            .last_token_form
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|form| form.get("resource")),
        Some(&mcp_url)
    );
    let manager = coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-refreshed-token"
    );

    fixture.reject_refresh.store(true, Ordering::SeqCst);
    let refresh_error = coordinator.refresh(instance_id, &mcp_url, &auth).await;
    assert!(matches!(refresh_error, Err(AuthError::RefreshFailed)));
    assert_eq!(fixture.refresh_requests.load(Ordering::SeqCst), 2);
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Unauthenticated
    );

    let error = match coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
    {
        Ok(_) => panic!("cleared credentials unexpectedly remained usable"),
        Err(error) => error,
    };
    assert!(matches!(error, AuthError::Required(_)));
}

#[tokio::test]
async fn concurrent_authorizations_keep_state_and_tokens_isolated_by_instance() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let first_id = instance_id("oauth-first");
    let second_id = instance_id("oauth-second");
    let auth = authorization_code_config();
    let mcp_url = fixture.mcp_url();

    let first = coordinator
        .begin_authorization(first_id, &mcp_url, &auth)
        .await
        .unwrap();
    let second = coordinator
        .begin_authorization(second_id, &mcp_url, &auth)
        .await
        .unwrap();
    let first_state = authorization_state_from_url(&first.authorization_url);
    let second_state = authorization_state_from_url(&second.authorization_url);
    assert_ne!(first_state, second_state);

    let crossed = coordinator
        .complete_authorization_callback(
            first_id,
            "valid-code",
            &second_state,
            Some(&fixture.base_url),
        )
        .await;
    assert!(matches!(crossed, Err(AuthError::CallbackRejected)));
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 0);

    coordinator
        .complete_authorization_callback(
            second_id,
            "valid-code",
            &second_state,
            Some(&fixture.base_url),
        )
        .await
        .unwrap();
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        coordinator.status(first_id).await,
        AuthStatus::Unauthenticated
    );
    assert_eq!(
        coordinator.status(second_id).await,
        AuthStatus::Authenticated
    );

    let first_error = match coordinator
        .prepare_http_authorization(first_id, &mcp_url, &auth)
        .await
    {
        Ok(_) => panic!("first instance unexpectedly reused second instance token"),
        Err(error) => error,
    };
    assert!(matches!(first_error, AuthError::Required(_)));
    let second_manager = coordinator
        .prepare_http_authorization(second_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        second_manager.get_access_token().await.unwrap(),
        "fixture-access-token"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_credentials_logout_clears_tokens_but_preserves_machine_credentials() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("client-credentials-logout");
    let auth = client_credentials_config();
    let mcp_url = fixture.mcp_url();

    coordinator
        .save_client_secret(
            instance_id,
            &mcp_url,
            &auth,
            ClientSecret::new("machine-secret"),
        )
        .await
        .unwrap();
    coordinator
        .save_private_key(
            instance_id,
            &mcp_url,
            &auth,
            PrivateKey::new(b"retained-private-key".to_vec()),
        )
        .await
        .unwrap();
    coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();

    coordinator
        .logout(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Unauthenticated
    );

    let key = AuthCredentialKey::new(
        instance_id,
        Some(mcp_url.clone()),
        None,
        "machine-client",
        ["tools.call".to_string()],
        None,
    );
    assert_eq!(
        coordinator
            .client_secret_store(&key)
            .load()
            .await
            .unwrap()
            .unwrap()
            .expose(),
        "machine-secret"
    );
    assert_eq!(
        coordinator
            .private_key_store(&key)
            .load()
            .await
            .unwrap()
            .unwrap()
            .expose(),
        b"retained-private-key"
    );

    let manager = coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        manager.get_access_token().await.unwrap(),
        "fixture-access-token"
    );
    assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn scope_upgrade_and_logout_have_predictable_lifecycle() {
    let fixture = OAuthFixture::spawn().await;
    let coordinator = AuthCoordinator::for_tests(test_keyring()).unwrap();
    let instance_id = instance_id("scope-upgrade");
    let auth = authorization_code_config();
    let mcp_url = fixture.mcp_url();
    complete_fixture_authorization(&coordinator, &fixture, instance_id, &auth).await;

    let upgrade = coordinator
        .begin_scope_upgrade(instance_id, &mcp_url, &auth, "tools.admin")
        .await
        .unwrap();
    assert!(upgrade.scopes.contains(&"tools.read".to_string()));
    assert!(upgrade.scopes.contains(&"tools.admin".to_string()));
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Authorizing
    );
    let upgrade_state = authorization_state_from_url(&upgrade.authorization_url);
    coordinator
        .complete_authorization_callback(
            instance_id,
            "valid-code",
            &upgrade_state,
            Some(&fixture.base_url),
        )
        .await
        .unwrap();

    coordinator
        .logout(instance_id, &mcp_url, &auth)
        .await
        .unwrap();
    assert_eq!(
        coordinator.status(instance_id).await,
        AuthStatus::Unauthenticated
    );
    let error = match coordinator
        .prepare_http_authorization(instance_id, &mcp_url, &auth)
        .await
    {
        Ok(_) => panic!("logout unexpectedly left OAuth credentials available"),
        Err(error) => error,
    };
    assert!(matches!(error, AuthError::Required(_)));
}
