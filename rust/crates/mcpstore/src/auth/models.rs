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
    Error,
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
}
