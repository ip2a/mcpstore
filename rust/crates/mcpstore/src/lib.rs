#![recursion_limit = "256"]

pub(crate) mod agent;
pub mod auth;
pub mod cache;
pub mod config;
pub mod config_formats;
pub mod client_config;
pub(crate) mod control;
pub mod event_reactor;
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
pub mod state;
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

// Facade re-exports: event reactor
pub use event_reactor::{
    ChangeContext, EventBackend, EventReactor, ReactionContext, ReactionOutcome, ReactorConfig,
    Rule,
};

// Facade re-exports: service instance identity
pub use identity::{InstanceId, ScopeRef, ServiceInstanceKey};

// Facade re-exports: registry
pub use registry::{ConfigRevision, ServiceDefinition, ServiceInstance, ToolInfo};

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
    McpCompletion, McpCompletionReference, McpCompletionRequest, McpElicitationRequest,
    McpElicitationRequestKind, McpElicitationResponseError, McpElicitationSession,
    McpElicitationSessionOptions, McpExecutionOptions, McpExecutionProgress, McpExecutionUpdate,
    McpLoggingLevel, McpServerCapabilities, McpServerImplementation, McpServerMetadata, McpTask,
    McpTaskRecord, McpTaskStatus, McpToolExecution, McpToolExecutionHandle, ToolCallResult,
};

// Facade re-exports: execution service
pub use service::{McpStoreExecutionUpdate, McpStoreToolExecutionHandle};

// Facade re-exports: service state
pub use state::{
    AuthState, DesiredState, FailureInfo, FailurePhase, HealthMetrics, HealthState, Readiness, ReadinessReason,
    ReadinessStatus, RecoveryState, RuntimePhase, ServiceState, ServiceStateError,
    ServiceStateEvent, ServiceStateManager, ServiceStateManagerError, ToolAvailability, ToolStateItem, ToolsState, ToolsStatus,
};

// Facade re-exports: core store
pub use core::{Result, StoreError};
pub use store::{
    BackendKind, CacheStorage, MCPStore, OpenApiImportInput, OpenApiImportSource, SourceMode,
    StoreOptions, ToolVisibilityFilter,
};
pub use tool_transform::ToolTransformPatch;
