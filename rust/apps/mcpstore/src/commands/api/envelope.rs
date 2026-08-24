use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use mcpstore::error::{Error, FailureCode};
use mcpstore::ErrorContext;
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

    pub(super) fn from_store(error: Error) -> Self {
        match error.code() {
            FailureCode::ConnectionAuthRequired => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTH_REQUIRED",
                error.message().to_string(),
                None,
                serde_json::to_value(error.context()).ok(),
            ),
            FailureCode::ConnectionScope => Self::new(
                StatusCode::FORBIDDEN,
                "AUTH_INSUFFICIENT_SCOPE",
                "OAuth 授权范围不足，需要升级授权",
                None,
                Some(json!({
                    "instance_id": match error.context() {
                        ErrorContext::Scope { instance_id, .. } => json!(instance_id),
                        _ => serde_json::Value::Null,
                    },
                    "required_scope": match error.context() {
                        ErrorContext::Scope { required_scope, .. } => json!(required_scope),
                        _ => serde_json::Value::Null,
                    },
                })),
            ),
            FailureCode::CapabilityUnsupported => Self::new(
                StatusCode::CONFLICT,
                "MCP_CAPABILITY_UNSUPPORTED",
                format!("远端 MCP 服务不支持 capability（{}）", error.message()),
                None,
                Some(json!({ "message": error.message() })),
            ),
            FailureCode::InvalidInput => Self::new(
                StatusCode::BAD_REQUEST,
                "MCP_INVALID_INPUT",
                error.message().to_string(),
                None,
                None,
            ),
            FailureCode::ServiceNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "SERVICE_NOT_FOUND",
                format!("服务不存在: {}", error.message()),
                Some("service_name"),
                None,
            ),
            FailureCode::ToolNotAvailable => Self::new(
                StatusCode::FORBIDDEN,
                "TOOL_NOT_AVAILABLE",
                error.message().to_string(),
                Some("tool_name"),
                Some(match error.context() {
                    ErrorContext::Tool {
                        instance_id,
                        tool_name,
                    } => json!({ "instance_id": instance_id, "tool_name": tool_name }),
                    _ => serde_json::Value::Null,
                }),
            ),
            FailureCode::SecureStorageUnavailable => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SECURE_STORAGE_UNAVAILABLE",
                "安全凭证存储不可用",
                None,
                None,
            ),
            FailureCode::OauthProviderFailed => Self::new(
                StatusCode::BAD_GATEWAY,
                "OAUTH_PROVIDER_FAILED",
                "OAuth 提供方操作失败",
                None,
                None,
            ),
            FailureCode::ConfigInvalid => Self::new(
                StatusCode::BAD_REQUEST,
                "CONFIG_INVALID",
                error.message().to_string(),
                None,
                None,
            ),
            FailureCode::AuthFailed => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTHENTICATION_ERROR",
                error.message().to_string(),
                None,
                Some(json!({ "error_type": "AuthError" })),
            ),
            FailureCode::SessionNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "SESSION_NOT_FOUND",
                error.message().to_string(),
                Some("session_key"),
                None,
            ),
            FailureCode::SessionNotActive => Self::new(
                StatusCode::CONFLICT,
                "SESSION_NOT_ACTIVE",
                error.message().to_string(),
                Some("session_key"),
                None,
            ),
            FailureCode::TaskStateFailed => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                error.message().to_string(),
                None,
                Some(json!({ "error_type": "CacheError" })),
            ),
            FailureCode::Internal => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                error.message().to_string(),
                None,
                None,
            ),
            _ => Self::new(
                StatusCode::BAD_GATEWAY,
                "SERVICE_OPERATION_FAILED",
                error.to_string(),
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
    use mcpstore::error::{Error, ErrorContext, FailureCode};
    use mcpstore::InstanceId;

    #[test]
    fn insufficient_scope_maps_to_http_403_with_stable_error_code() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        let error = ApiError::from_store(
            Error::new(FailureCode::ConnectionScope, "insufficient OAuth scope").with_context(
                ErrorContext::Scope {
                    instance_id,
                    required_scope: Some("resources.read tools.call".to_string()),
                },
            ),
        );

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
        let error = ApiError::from_store(
            Error::new(
                FailureCode::CapabilityUnsupported,
                "MCP service instance does not support capability completions",
            )
            .with_context(ErrorContext::Service {
                instance_id,
                service_name: String::new(),
            }),
        );

        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "MCP_CAPABILITY_UNSUPPORTED");
        assert!(error
            .details
            .as_ref()
            .and_then(|details| details.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("completions")));
    }
}
