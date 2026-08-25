use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use mcpstore::error::{Error, FailureCode};
use serde::Serialize;
use serde_json::Value;

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
        code: FailureCode,
        message: impl Into<String>,
        field: Option<&str>,
        details: Option<Value>,
    ) -> Self {
        Self {
            status,
            code: code.as_str().to_string(),
            message: message.into(),
            field: field.map(ToString::to_string),
            details,
        }
    }

    pub(super) fn missing_parameter(field: &'static str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            FailureCode::InvalidInput,
            format!("缺少 {field}"),
            Some(field),
            None,
        )
    }

    pub(super) fn invalid_parameter(message: impl Into<String>, field: Option<&str>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            FailureCode::InvalidInput,
            message,
            field,
            None,
        )
    }

    pub(super) fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            FailureCode::InvalidInput,
            message,
            None,
            None,
        )
    }

    pub(super) fn not_found(
        code: FailureCode,
        message: impl Into<String>,
        field: Option<&str>,
        details: Option<Value>,
    ) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, field, details)
    }

    pub(super) fn from_store(error: Error) -> Self {
        Self::new(
            http_status(error.code()),
            error.code(),
            error.message().to_string(),
            None,
            serde_json::to_value(error.context()).ok(),
        )
    }
}

/// Failure code → HTTP status, the single mapping used by the API layer.
pub(super) fn http_status(code: FailureCode) -> StatusCode {
    use FailureCode as Code;
    use StatusCode as Http;
    match code {
        Code::InvalidInput | Code::ConfigInvalid | Code::ConnectionUnsupported => Http::BAD_REQUEST,
        Code::ServiceNotFound
        | Code::TaskNotFound
        | Code::SessionNotFound
        | Code::ToolNotAvailable => Http::NOT_FOUND,
        Code::ConnectionAuthRequired => Http::UNAUTHORIZED,
        Code::ConnectionScope => Http::FORBIDDEN,
        Code::CallCancelled
        | Code::CapabilityUnsupported
        | Code::TaskNotCancellable
        | Code::ElicitationCancelled
        | Code::SessionNotActive => Http::CONFLICT,
        Code::ElicitationInputRequired => StatusCode::from_u16(428).unwrap_or(Http::CONFLICT),
        Code::ConnectionTimedOut | Code::CallTimedOut | Code::ElicitationTimedOut => {
            Http::GATEWAY_TIMEOUT
        }
        Code::ServiceUnavailable
        | Code::HealthCheckFailed
        | Code::ProbeTimedOut
        | Code::ToolSyncFailed
        | Code::OpenapiRequestFailed => Http::SERVICE_UNAVAILABLE,
        Code::AuthFailed
        | Code::SecureStorageUnavailable
        | Code::TaskStateFailed
        | Code::StopFailed
        | Code::Internal => Http::INTERNAL_SERVER_ERROR,
        // Everything below means "the upstream MCP service is unreachable or
        // misbehaving in a way the client can't fix by editing its request".
        // Listed explicitly (not `_ =>`) so a new FailureCode variant fails to
        // compile here until someone picks a status for it on purpose.
        Code::ConnectionSpawnFailed
        | Code::ConnectionRefused
        | Code::ConnectionTls
        | Code::ConnectionClosed
        | Code::HandshakeIncompatible
        | Code::HandshakeRejected
        | Code::HandshakeUncorrelated
        | Code::HandshakeFailed
        | Code::NotConnected
        | Code::ToolFailed
        | Code::CallDisconnected
        | Code::TaskUnavailable
        | Code::TaskFailed
        | Code::OauthProviderFailed
        | Code::ElicitationInvalidResponse => Http::BAD_GATEWAY,
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
    use mcpstore::error::ErrorContext;
    use mcpstore::InstanceId;

    #[test]
    fn insufficient_scope_maps_to_http_403_with_failure_code() {
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
        assert_eq!(error.code, "connection_scope");
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
    fn unsupported_capability_maps_to_http_409_with_failure_code() {
        let error = ApiError::from_store(Error::new(
            FailureCode::CapabilityUnsupported,
            "MCP service instance does not support capability completions",
        ));

        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "capability_unsupported");
    }

    #[test]
    fn failure_codes_map_to_documented_http_statuses() {
        for (code, expected) in [
            (FailureCode::HandshakeIncompatible, StatusCode::BAD_GATEWAY),
            (
                FailureCode::ConnectionAuthRequired,
                StatusCode::UNAUTHORIZED,
            ),
            (FailureCode::SessionNotFound, StatusCode::NOT_FOUND),
            (FailureCode::SessionNotActive, StatusCode::CONFLICT),
        ] {
            assert_eq!(http_status(code), expected, "{code}");
        }
    }

    #[test]
    fn every_failure_code_has_an_http_status() {
        for code in FailureCode::ALL {
            let status = http_status(code);
            assert!(
                (400..=599).contains(&status.as_u16()),
                "{code} mapped to {status}"
            );
        }
    }
}
