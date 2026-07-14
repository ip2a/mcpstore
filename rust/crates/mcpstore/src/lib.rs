#![recursion_limit = "256"]

pub(crate) mod agent;
pub mod auth;
pub mod cache;
pub mod config;
pub mod config_formats;
pub(crate) mod control;
pub mod core;
pub mod events;
pub(crate) mod health;
pub mod identity;
#[cfg(feature = "mcp-server")]
pub mod mcp_server;
pub mod openapi;
pub mod openapi_runtime;
pub mod perspective;
pub mod registry;
pub(crate) mod service;
pub mod session;
pub mod store;
pub mod tool_transform;
pub mod transport;

// Facade re-exports: configuration
pub use auth::{
    AuthConfig, AuthCredentialKey, AuthError, AuthFlow, AuthRequired, AuthStatus, AuthStatusView,
    AuthorizationCodeClientAuthMethod, AuthorizationStart, ClientCredentialsAuthMethod,
    ClientSecret, JwtSigningAlgorithm, KeyringClientSecretStore, KeyringCredentialStore,
    KeyringPrivateKeyStore, KeyringStateStore, OAuthAuthorizationCodeConfig,
    OAuthClientCredentialsConfig, PrivateKey,
};
pub use config::{AppConfig, CacheBackend, CacheConfig, ConfigManager, McpConfig, ServerConfig};

// Facade re-exports: cache layer
pub use cache::{CacheLayerManager, CacheSnapshot};

// Facade re-exports: OpenAPI runtime imports
pub use openapi::{
    OpenApiBundleArtifact, OpenApiBundleDependency, OpenApiBundleDiagnostic, OpenApiBundleDocument,
    OpenApiBundleOptions, OpenApiComponent, OpenApiComponentCounts, OpenApiComponentType,
    OpenApiEndpoint, OpenApiImportOptions, OpenApiImportResult, OpenApiRefCachePolicy,
    OpenApiSpecInfo,
};

// Facade re-exports: event bus
pub use events::{Event, EventBus};

// Facade re-exports: service instance identity
pub use identity::{InstanceId, ScopeRef, ServiceInstanceKey};

// Facade re-exports: registry
pub use registry::{
    ConfigRevision, ConnectionStatus, ServiceDefinition, ServiceInstance, ToolInfo,
};

// Facade re-exports: business sessions
pub use cache::models::{
    SessionContextState, SessionEntity, SessionScope, SessionServiceItem, SessionServiceRelation,
    SessionStateData, SessionStatus, SessionStatusState, SessionToolItem, SessionToolVisibility,
    ToolArgumentTransform, ToolPreferenceState, ToolTransformRule, ToolTransformSafetyPolicy,
};
pub use session::{
    CreateSessionRequest, SessionBuilder, SessionCleanupReport, SessionContext,
    SessionImportReport, SessionRestartReport, SessionRetryPolicy, SessionToolSelection,
};

// Facade re-exports: transport
pub use transport::{
    ContentItem, DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate, DiscoveredTool,
    McpCompletion, McpCompletionReference, McpCompletionRequest, McpLoggingLevel,
    McpServerCapabilities, McpServerImplementation, McpServerMetadata, ToolCallResult,
};

// Facade re-exports: core store
pub use core::{Result, StoreError};
pub use store::{
    BackendKind, CacheStorage, MCPStore, OpenApiImportInput, OpenApiImportSource, SourceMode,
    StoreOptions, ToolVisibilityFilter,
};
pub use tool_transform::ToolTransformPatch;
