pub use crate::perspective;
pub use crate::store;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Authentication error: {0}")]
    Auth(#[from] crate::auth::AuthError),
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("Transport error: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("Cache error: {0}")]
    Cache(#[from] crate::cache::CacheError),
    #[error("Service state error: {0}")]
    State(#[from] crate::state::ServiceStateManagerError),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("Tool is not available: instance_id={instance_id}, tool_name={tool_name}")]
    ToolNotAvailable {
        instance_id: crate::identity::InstanceId,
        tool_name: String,
    },
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
