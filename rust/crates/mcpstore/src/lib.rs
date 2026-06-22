#![recursion_limit = "256"]

pub(crate) mod agent;
pub mod cache;
pub mod config;
pub(crate) mod control;
pub mod core;
pub mod events;
pub(crate) mod health;
pub mod perspective;
pub mod registry;
pub(crate) mod service;
pub mod store;
pub mod transport;

// Facade re-exports: configuration
pub use config::{AppConfig, CacheBackend, CacheConfig, ConfigManager, McpConfig, ServerConfig};

// Facade re-exports: cache layer
pub use cache::{CacheLayerManager, CacheSnapshot, KvStore, MemoryStore, RedisStore};

// Facade re-exports: event bus
pub use events::{Event, EventBus};

// Facade re-exports: registry
pub use registry::{ConnectionStatus, ServiceEntry, ToolInfo};

// Facade re-exports: transport
pub use transport::{ContentItem, ToolCallResult, ToolDescription};

// Facade re-exports: core store
pub use core::{Result, StoreError};
pub use store::{BackendKind, CacheStorage, MCPStore, SourceMode, StoreOptions};
