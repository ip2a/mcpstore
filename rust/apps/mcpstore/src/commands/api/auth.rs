use super::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct AuthCallbackQuery {
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

pub(super) async fn store_auth_status(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("认证状态获取成功", json!({ "auth": auth })))
}

pub(super) async fn store_auth_start(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    match auth.flow {
        Some(AuthFlow::AuthorizationCode) => {
            let authorization = state
                .store
                .begin_authorization(instance_id)
                .await
                .map_err(ApiError::from_store)?;
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

pub(super) async fn store_auth_callback_get(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(query): Query<AuthCallbackQuery>,
) -> ApiResult {
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

pub(super) async fn store_auth_callback_post(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthCallbackRequest>,
) -> ApiResult {
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

pub(super) async fn store_auth_refresh(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
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

pub(super) async fn store_auth_logout(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
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

pub(super) async fn store_auth_save_client_secret(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthClientSecretRequest>,
) -> ApiResult {
    state
        .store
        .save_oauth_client_secret(instance_id, payload.client_secret)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("客户端密钥已安全保存", json!({ "stored": true })))
}

pub(super) async fn store_auth_save_private_key(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthPrivateKeyRequest>,
) -> ApiResult {
    state
        .store
        .save_oauth_private_key(instance_id, payload.private_key_pem.into_bytes())
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("私钥已安全保存", json!({ "stored": true })))
}

pub(super) async fn store_auth_scope_upgrade(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthScopeUpgradeRequest>,
) -> ApiResult {
    let authorization = state
        .store
        .begin_scope_upgrade(instance_id, &payload.required_scope)
        .await
        .map_err(ApiError::from_store)?;
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
