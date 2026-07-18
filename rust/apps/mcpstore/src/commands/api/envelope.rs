use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use mcpstore::StoreError;
use serde::Serialize;
use serde_json::{json, Value};

const API_VERSION: &str = "1.0.0";

#[derive(Serialize)]
pub(super) struct ApiMeta {
    timestamp: String,
    request_id: String,
    execution_time_ms: i64,
    api_version: &'static str,
}

#[derive(Serialize)]
pub(super) struct ApiErrorDetail {
    code: String,
    message: String,
    field: Option<String>,
    details: Option<Value>,
}

#[derive(Serialize)]
pub(super) struct ApiEnvelope {
    success: bool,
    message: String,
    data: Option<Value>,
    errors: Option<Vec<ApiErrorDetail>>,
    meta: ApiMeta,
    pagination: Option<Value>,
}

#[derive(Debug)]
pub(super) struct ApiError {
    pub(super) status: StatusCode,
    pub(super) code: String,
    message: String,
    field: Option<String>,
    details: Option<Value>,
}

impl ApiError {
    fn new(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        field: Option<&str>,
        details: Option<Value>,
    ) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            field: field.map(ToString::to_string),
            details,
        }
    }

    pub(super) fn missing_parameter(field: &'static str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "MISSING_PARAMETER",
            format!("缺少 {field}"),
            Some(field),
            None,
        )
    }

    pub(super) fn invalid_parameter(message: impl Into<String>, field: Option<&str>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_PARAMETER",
            message,
            field,
            None,
        )
    }

    pub(super) fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            message,
            None,
            None,
        )
    }

    pub(super) fn not_found(
        code: impl Into<String>,
        message: impl Into<String>,
        field: Option<&str>,
        details: Option<Value>,
    ) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, field, details)
    }

    pub(super) fn from_store(error: StoreError) -> Self {
        match error {
            StoreError::Auth(mcpstore::AuthError::Required(required)) => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTH_REQUIRED",
                required.to_string(),
                None,
                serde_json::to_value(required).ok(),
            ),
            StoreError::Auth(mcpstore::AuthError::InvalidConfig(message)) => Self::new(
                StatusCode::BAD_REQUEST,
                "AUTH_CONFIG_INVALID",
                message,
                None,
                None,
            ),
            StoreError::Auth(mcpstore::AuthError::CallbackRejected) => Self::new(
                StatusCode::BAD_REQUEST,
                "AUTH_CALLBACK_REJECTED",
                "授权回调校验失败",
                None,
                None,
            ),
            StoreError::Auth(mcpstore::AuthError::RefreshFailed) => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTH_REFRESH_FAILED",
                "令牌刷新失败，需要重新授权",
                None,
                None,
            ),
            StoreError::Auth(mcpstore::AuthError::MissingClientCredential) => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTH_CREDENTIAL_REQUIRED",
                "需要先写入 OAuth 客户端凭证",
                None,
                None,
            ),
            StoreError::Auth(mcpstore::AuthError::UnsupportedFlow) => Self::new(
                StatusCode::CONFLICT,
                "AUTH_FLOW_UNSUPPORTED",
                "当前认证流程不支持此操作",
                None,
                None,
            ),
            StoreError::Auth(mcpstore::AuthError::SecureStorage { .. }) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SECURE_STORAGE_UNAVAILABLE",
                "安全凭证存储不可用",
                None,
                None,
            ),
            StoreError::Auth(
                mcpstore::AuthError::AuthorizationStartFailed
                | mcpstore::AuthError::ProviderFailure,
            ) => Self::new(
                StatusCode::BAD_GATEWAY,
                "OAUTH_PROVIDER_FAILED",
                "OAuth 提供方操作失败",
                None,
                None,
            ),
            StoreError::Auth(error) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTHENTICATION_ERROR",
                error.to_string(),
                None,
                Some(json!({ "error_type": "AuthError" })),
            ),
            StoreError::ServiceNotFound(name) => Self::new(
                StatusCode::NOT_FOUND,
                "SERVICE_NOT_FOUND",
                format!("服务不存在: {name}"),
                Some("service_name"),
                Some(json!({ "service_name": name })),
            ),
            StoreError::Config(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "CONFIG_INVALID",
                error.to_string(),
                None,
                Some(json!({ "error_type": "ConfigError" })),
            ),
            StoreError::Transport(mcpstore::transport::TransportError::AuthRequired(required)) => {
                Self::new(
                    StatusCode::UNAUTHORIZED,
                    "AUTH_REQUIRED",
                    required.to_string(),
                    None,
                    serde_json::to_value(required).ok(),
                )
            }
            StoreError::Transport(mcpstore::transport::TransportError::InsufficientScope {
                instance_id,
                required_scope,
            }) => Self::new(
                StatusCode::FORBIDDEN,
                "AUTH_INSUFFICIENT_SCOPE",
                "OAuth 授权范围不足，需要升级授权",
                None,
                Some(json!({
                    "instance_id": instance_id,
                    "required_scope": required_scope,
                })),
            ),
            StoreError::Transport(mcpstore::transport::TransportError::CapabilityUnsupported {
                instance_id,
                capability,
            }) => Self::new(
                StatusCode::CONFLICT,
                "MCP_CAPABILITY_UNSUPPORTED",
                format!("远端 MCP 服务不支持 {capability} capability"),
                None,
                Some(json!({
                    "instance_id": instance_id,
                    "capability": capability,
                })),
            ),
            StoreError::Transport(mcpstore::transport::TransportError::InvalidInput(message)) => {
                Self::new(
                    StatusCode::BAD_REQUEST,
                    "MCP_INVALID_INPUT",
                    message,
                    None,
                    None,
                )
            }
            StoreError::Transport(error) => Self::new(
                StatusCode::BAD_GATEWAY,
                "SERVICE_OPERATION_FAILED",
                error.to_string(),
                None,
                Some(json!({ "error_type": "TransportError" })),
            ),
            StoreError::Cache(error) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                error.to_string(),
                None,
                Some(json!({ "error_type": "CacheError" })),
            ),
            StoreError::Other(message) if message.contains("Session not found") => Self::new(
                StatusCode::NOT_FOUND,
                "SESSION_NOT_FOUND",
                message,
                Some("session_key"),
                None,
            ),
            StoreError::Other(message) if message.contains("Session is not active") => Self::new(
                StatusCode::CONFLICT,
                "SESSION_NOT_ACTIVE",
                message,
                Some("session_key"),
                None,
            ),
            StoreError::State(error) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STATE_ERROR",
                error.to_string(),
                None,
                None,
            ),
            StoreError::Other(message) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                message,
                None,
                None,
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let payload = ApiEnvelope {
            success: false,
            message: self.message.clone(),
            data: None,
            errors: Some(vec![ApiErrorDetail {
                code: self.code,
                message: self.message,
                field: self.field,
                details: self.details,
            }]),
            meta: api_meta(),
            pagination: None,
        };
        (self.status, Json(payload)).into_response()
    }
}

pub(super) type ApiResult<T = Json<ApiEnvelope>> = Result<T, ApiError>;

pub(super) fn success(message: impl Into<String>, data: Value) -> Json<ApiEnvelope> {
    Json(ApiEnvelope {
        success: true,
        message: message.into(),
        data: Some(data),
        errors: None,
        meta: api_meta(),
        pagination: None,
    })
}

fn api_meta() -> ApiMeta {
    ApiMeta {
        timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        request_id: format!(
            "req_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        execution_time_ms: 0,
        api_version: API_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::transport::TransportError;
    use mcpstore::{InstanceId, StoreError};

    #[test]
    fn insufficient_scope_maps_to_http_403_with_stable_error_code() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        let error =
            ApiError::from_store(StoreError::Transport(TransportError::InsufficientScope {
                instance_id,
                required_scope: Some("resources.read tools.call".to_string()),
            }));

        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "AUTH_INSUFFICIENT_SCOPE");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("required_scope"))
                .and_then(Value::as_str),
            Some("resources.read tools.call")
        );
    }

    #[test]
    fn unsupported_capability_maps_to_http_409_with_stable_error_code() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        let error = ApiError::from_store(StoreError::Transport(
            TransportError::CapabilityUnsupported {
                instance_id,
                capability: "completions",
            },
        ));

        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "MCP_CAPABILITY_UNSUPPORTED");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("capability"))
                .and_then(Value::as_str),
            Some("completions")
        );
    }
}
