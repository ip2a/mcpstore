//! Event type definitions and helpers.
//!
//! Provides strongly-typed builder functions for common MCPStore events.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct EventCapabilityReport {
    pub event_bus: bool,
    pub history: bool,
    pub history_capacity: usize,
    pub cache_event_layer: bool,
}

/// Service lifecycle event kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    ServiceAddRequested,
    ServiceBootstrapRequested,
    ServiceBootstrapped,
    ServiceBootstrapFailed,
    ServiceCached,
    ServiceInitialized,
    ServiceConnectionRequested,
    ServiceConnected,
    ServiceConnectionFailed,
    ServicePersisting,
    ServicePersisted,
    ServiceReady,
    ServiceOperationFailed,
    ServiceStateChanged,
    HealthCheckRequested,
    HealthCheckCompleted,
    ServiceTimeout,
    ServiceRestartRequested,
    ServiceResetRequested,
    ReconnectionRequested,
    ReconnectionScheduled,
    ToolSyncStarted,
    ToolSyncCompleted,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventKind::ServiceAddRequested => "SERVICE_ADD_REQUESTED",
            EventKind::ServiceBootstrapRequested => "SERVICE_BOOTSTRAP_REQUESTED",
            EventKind::ServiceBootstrapped => "SERVICE_BOOTSTRAPPED",
            EventKind::ServiceBootstrapFailed => "SERVICE_BOOTSTRAP_FAILED",
            EventKind::ServiceCached => "SERVICE_CACHED",
            EventKind::ServiceInitialized => "SERVICE_INITIALIZED",
            EventKind::ServiceConnectionRequested => "SERVICE_CONNECTION_REQUESTED",
            EventKind::ServiceConnected => "SERVICE_CONNECTED",
            EventKind::ServiceConnectionFailed => "SERVICE_CONNECTION_FAILED",
            EventKind::ServicePersisting => "SERVICE_PERSISTING",
            EventKind::ServicePersisted => "SERVICE_PERSISTED",
            EventKind::ServiceReady => "SERVICE_READY",
            EventKind::ServiceOperationFailed => "SERVICE_OPERATION_FAILED",
            EventKind::ServiceStateChanged => "SERVICE_STATE_CHANGED",
            EventKind::HealthCheckRequested => "HEALTH_CHECK_REQUESTED",
            EventKind::HealthCheckCompleted => "HEALTH_CHECK_COMPLETED",
            EventKind::ServiceTimeout => "SERVICE_TIMEOUT",
            EventKind::ServiceRestartRequested => "SERVICE_RESTART_REQUESTED",
            EventKind::ServiceResetRequested => "SERVICE_RESET_REQUESTED",
            EventKind::ReconnectionRequested => "RECONNECTION_REQUESTED",
            EventKind::ReconnectionScheduled => "RECONNECTION_SCHEDULED",
            EventKind::ToolSyncStarted => "TOOL_SYNC_STARTED",
            EventKind::ToolSyncCompleted => "TOOL_SYNC_COMPLETED",
        }
    }
}
