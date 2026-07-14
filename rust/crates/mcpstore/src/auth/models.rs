use serde::{Deserialize, Serialize};

use crate::identity::InstanceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    NotRequired,
    Unauthenticated,
    Authorizing,
    Authenticated,
    Refreshing,
    ScopeUpgradeRequired,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthStatusView {
    pub instance_id: InstanceId,
    pub status: AuthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<AuthFlow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationStart {
    pub instance_id: InstanceId,
    pub authorization_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthRequired {
    pub instance_id: InstanceId,
    pub flow: AuthFlow,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

impl std::fmt::Display for AuthRequired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "authentication required for service instance {}",
            self.instance_id
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlow {
    AuthorizationCode,
    ClientCredentials,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authentication required")]
    Required(AuthRequired),
    #[error("secure credential storage is unavailable during {operation}: {kind}")]
    SecureStorage {
        operation: &'static str,
        kind: &'static str,
    },
    #[error("stored authentication data is invalid")]
    InvalidStoredData,
    #[error("authentication configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("authorization cannot start")]
    AuthorizationStartFailed,
    #[error("authorization callback was rejected")]
    CallbackRejected,
    #[error("token refresh failed; authorization is required again")]
    RefreshFailed,
    #[error("client credentials are not available in secure storage")]
    MissingClientCredential,
    #[error("authentication operation is not supported for this flow")]
    UnsupportedFlow,
    #[error("authentication provider operation failed")]
    ProviderFailure,
}
