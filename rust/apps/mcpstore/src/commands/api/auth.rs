use super::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use mcpstore::{InstanceId, LocalCallbackListener, MCPStore};
use serde::Deserialize;

const OAUTH_CALLBACK_TIMEOUT_SECS: u64 = 300;

/// OAuth 回调（GET）：浏览器跳转带来 `code/state/iss`，同时携带 `scope/agent_id` 用于定位服务。
#[derive(Deserialize)]
pub(super) struct AuthCallbackQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    code: Option<String>,
    state: Option<String>,
    #[serde(rename = "iss")]
    issuer: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AuthCallbackRequest {
    callback_url: String,
}

#[derive(Deserialize)]
pub(super) struct AuthClientSecretRequest {
    client_secret: String,
}

#[derive(Deserialize)]
pub(super) struct AuthPrivateKeyRequest {
    private_key_pem: String,
}

#[derive(Deserialize)]
pub(super) struct AuthScopeUpgradeRequest {
    required_scope: String,
}

pub(super) async fn service_auth_status(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("认证状态获取成功", json!({ "auth": auth })))
}

pub(super) async fn service_auth_start(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    match auth.flow {
        Some(AuthFlow::AuthorizationCode) => {
            let callback_uri = state
                .store
                .authorization_callback_uri(instance_id)
                .await
                .map_err(ApiError::from_store)?
                .ok_or_else(|| {
                    ApiError::invalid_request("Authorization Code flow has no callback URI")
                })?;
            let listener = LocalCallbackListener::bind(&callback_uri)
                .await
                .map_err(ApiError::invalid_request)?;
            let authorization = state
                .store
                .begin_authorization(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            spawn_oauth_callback_task(state.store.clone(), instance_id, listener);
            let auth = state
                .store
                .auth_status_view(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            Ok(success(
                "授权已开始",
                json!({ "auth": auth, "authorization": authorization }),
            ))
        }
        Some(AuthFlow::ClientCredentials) => {
            state
                .store
                .refresh_authorization(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            reconnect_authorized_service(&state, instance_id).await?;
            let auth = state
                .store
                .auth_status_view(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            Ok(success(
                "客户端凭证授权成功",
                json!({ "auth": auth, "authorization": null }),
            ))
        }
        None => Err(ApiError::from_store(mcpstore::StoreError::Auth(
            mcpstore::AuthError::UnsupportedFlow,
        ))),
    }
}

pub(super) async fn service_auth_callback_get(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<AuthCallbackQuery>,
) -> ApiResult {
    let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let code = query
        .code
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("code"))?;
    let csrf_state = query
        .state
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("state"))?;
    state
        .store
        .complete_authorization_callback(instance_id, code, csrf_state, query.issuer.as_deref())
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权回调处理成功", json!({ "auth": auth })))
}

pub(super) async fn service_auth_callback_post(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<AuthCallbackRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    if payload.callback_url.trim().is_empty() {
        return Err(ApiError::invalid_parameter(
            "callback_url 不能为空",
            Some("callback_url"),
        ));
    }
    state
        .store
        .complete_authorization(instance_id, &payload.callback_url)
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权回调处理成功", json!({ "auth": auth })))
}

pub(super) async fn service_auth_refresh(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .refresh_authorization(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权刷新成功", json!({ "auth": auth })))
}

pub(super) async fn service_auth_logout(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .logout_authorization(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权已退出", json!({ "auth": auth })))
}

pub(super) async fn service_auth_save_client_secret(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<AuthClientSecretRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .save_oauth_client_secret(instance_id, payload.client_secret)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("客户端密钥已安全保存", json!({ "stored": true })))
}

pub(super) async fn service_auth_save_private_key(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<AuthPrivateKeyRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .save_oauth_private_key(instance_id, payload.private_key_pem.into_bytes())
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("私钥已安全保存", json!({ "stored": true })))
}

pub(super) async fn service_auth_scope_upgrade(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<AuthScopeUpgradeRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let callback_uri = state
        .store
        .authorization_callback_uri(instance_id)
        .await
        .map_err(ApiError::from_store)?
        .ok_or_else(|| {
            ApiError::invalid_request("Scope upgrade requires Authorization Code authentication")
        })?;
    let listener = LocalCallbackListener::bind(&callback_uri)
        .await
        .map_err(ApiError::invalid_request)?;
    let authorization = state
        .store
        .begin_scope_upgrade(instance_id, &payload.required_scope)
        .await
        .map_err(ApiError::from_store)?;
    spawn_oauth_callback_task(state.store.clone(), instance_id, listener);
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "权限范围升级授权已开始",
        json!({ "auth": auth, "authorization": authorization }),
    ))
}

fn spawn_oauth_callback_task(
    store: Arc<MCPStore>,
    instance_id: InstanceId,
    listener: LocalCallbackListener,
) {
    tokio::spawn(async move {
        let callback = match listener.wait(OAUTH_CALLBACK_TIMEOUT_SECS).await {
            Ok(callback) => callback,
            Err(error) => {
                tracing::warn!(
                    "OAuth callback listener failed for instance {instance_id}: {error}"
                );
                return;
            }
        };
        if store
            .complete_authorization_callback(
                instance_id,
                &callback.code,
                &callback.state,
                callback.issuer.as_deref(),
            )
            .await
            .is_err()
        {
            return;
        }
        store.disconnect_service(instance_id).await.ok();
        if let Err(error) = store.connect_service(instance_id).await {
            tracing::warn!(
                "Reconnect after OAuth callback failed for instance {instance_id}: {error}"
            );
        }
    });
}

pub(super) async fn reconnect_authorized_service(
    state: &Arc<ApiState>,
    instance_id: InstanceId,
) -> Result<(), ApiError> {
    state.store.disconnect_service(instance_id).await.ok();
    state
        .store
        .connect_service(instance_id)
        .await
        .map_err(ApiError::from_store)
}
