pub mod perspective;
pub mod store;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Config error: {0}")]
    Config(#[from] mcpstore_config::ConfigError),
    #[error("Transport error: {0}")]
    Transport(#[from] mcpstore_transport::TransportError),
    #[error("Cache error: {0}")]
    Cache(#[from] mcpstore_cache::CacheError),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
