#![recursion_limit = "256"]

pub mod cache;
pub mod config;
pub mod core;
pub mod events;
pub mod registry;
pub mod transport;

// Facade re-exports: configuration
pub use config::{
    AppConfig, CacheBackend, CacheConfig, ConfigManager, McpConfig, ServerConfig,
};

// Facade re-exports: cache layer
pub use cache::{
    CacheLayerManager, CacheSnapshot, KvStore, MemoryStore, RedisStore,
};

// Facade re-exports: event bus
pub use events::{Event, EventBus};

// Facade re-exports: registry
pub use registry::{ConnectionStatus, ServiceEntry, ToolInfo};

// Facade re-exports: transport
pub use transport::{ContentItem, ToolCallResult, ToolDescription};

// Facade re-exports: core store
pub use core::store::{BackendKind, MCPStore, SourceMode, StoreOptions};
pub use core::{perspective, StoreError, Result};
