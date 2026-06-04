pub mod perspective;
pub mod store;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("Transport error: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("Cache error: {0}")]
    Cache(#[from] crate::cache::CacheError),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
