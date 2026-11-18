//! Gateway: projects the store's services as one configurable MCP surface over
//! streamable-http (the only transport). What gets exposed and where it routes are setup
//! options, not MCP-serving boilerplate.

use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use mcpstore::{
    config::{
        GatewayCallsConfig, McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig,
    },
    events::{bus::EventHandler, Event},
    ContentItem, InstanceId, MCPStore, McpExecutionOptions, McpToolExecution, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, PromptOverridePatch, ResourceOverridePatch,
    ResourceTemplateOverridePatch, ScopeRef, ToolOverridePatch,
};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, Prompt, ProtocolVersion, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerConfig as McpServerConfig, SubscriptionFilter, Tool,
        ToolAnnotations,
    },
    service::{SubscriptionContext, SubscriptionSink},
    transport::{
        streamable_http_server::session::local::LocalSessionManager, StreamableHttpServerConfig,
        StreamableHttpService,
    },
    ErrorData, ServerHandler,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Alias for the dynamic error type used by the gateway runner.
pub type BoxErr = Box<dyn std::error::Error>;

/// Gateway exposure setup: which scope/instance is projected, where it binds (HTTP only),
/// and which management tool families are visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayOptions {
    pub scope: ScopeRef,
    pub instance_id: Option<InstanceId>,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub session_key: Option<String>,
    pub expose_session_state_tools: bool,
    pub expose_tool_override_tools: bool,
    pub expose_prompt_override_tools: bool,
    pub expose_resource_override_tools: bool,
    pub expose_openapi_tools: bool,
    pub expose_service_tools: bool,
    pub expose_kv_tools: bool,
    pub expose_search_tools: bool,
}

impl Default for GatewayOptions {
    fn default() -> Self {
        Self {
            scope: ScopeRef::Store,
            instance_id: None,
            host: "127.0.0.1".to_string(),
            port: 1828,
            path: "/mcp".to_string(),
            session_key: None,
            expose_session_state_tools: false,
            expose_tool_override_tools: false,
            expose_prompt_override_tools: false,
            expose_resource_override_tools: false,
            expose_openapi_tools: false,
            expose_service_tools: false,
            expose_kv_tools: false,
            expose_search_tools: false,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct GatewayLaunchDescriptor {
    pub url: String,
}

pub const GATEWAY_BOOT_PREFIX: &str = "[GATEWAY_BOOT]";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayBootPhase {
    Spawning,
    Mounting,
    Binding,
    Ready,
    Failed,
}

impl GatewayBootPhase {
    pub fn is_starting(self) -> bool {
        !matches!(self, Self::Ready | Self::Failed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GatewayBoot {
    pub phase: GatewayBootPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl GatewayBoot {
    pub fn new(phase: GatewayBootPhase) -> Self {
        Self {
            phase,
            done: None,
            total: None,
            message: None,
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            phase: GatewayBootPhase::Failed,
            done: None,
            total: None,
            message: None,
            error: Some(error.into()),
        }
    }
}

pub type GatewayBootReporter = Arc<dyn Fn(GatewayBoot) + Send + Sync>;

/// Machine-readable child boot progress on stderr; the daemon consumes these
/// lines. Runtime mount reconciliation must not reopen the completed boot.
pub(crate) fn report_gateway_boot(boot: GatewayBoot) {
    static BOOT_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    if !BOOT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let encoded = serde_json::to_string(&boot).expect("gateway boot progress serializes");
    eprintln!("{GATEWAY_BOOT_PREFIX} {encoded}");
    if matches!(
        boot.phase,
        GatewayBootPhase::Ready | GatewayBootPhase::Failed
    ) {
        BOOT_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// What kind of business call a routed record represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCallKind {
    Tool,
    Prompt,
    Resource,
}

/// Actual lifecycle state; a recent completion is never "calling".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCallPhase {
    Routing,
    CallingUpstream,
    Succeeded,
    Failed,
}

/// One routed business call through the gateway (management tools excluded).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GatewayCallRecord {
    pub seq: u64,
    pub call_id: String,
    pub phase: GatewayCallPhase,
    pub upstream_name: Option<String>,
    pub namespace_applied: bool,
    pub name_mapped: bool,
    pub error: Option<String>,
    pub routing_ms: Option<u64>,
    pub request_write_ms: Option<u64>,
    pub first_byte_ms: Option<u64>,
    pub response_read_ms: Option<u64>,
    pub at_unix_ms: u64,
    /// Mount the call came through (`/mcp/<id>`); None = the generic `/mcp`.
    pub caller: Option<String>,
    pub kind: GatewayCallKind,
    /// Tool/prompt name, or the resolved upstream URI for resources.
    pub name: String,
    pub instance_id: Option<InstanceId>,
    pub service_name: String,
    pub duration_ms: u64,
    pub success: bool,
}

/// Bounded in-memory ring of recent routed calls; read by the app-plane /gateway/requests
/// with `?after=<seq>` long-polling (watch wakes the waiter on each push).
pub struct GatewayCallLog {
    records: std::sync::Mutex<std::collections::VecDeque<GatewayCallRecord>>,
    latest: std::sync::atomic::AtomicU64,

    history: Option<Arc<history::GatewayCallHistory>>,
    config: GatewayCallsConfig,
    changed: tokio::sync::watch::Sender<u64>,
}

pub(crate) fn new_gateway_call_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

impl GatewayCallLog {
    pub fn new() -> Arc<Self> {
        Self::from_saved(Vec::new(), None, GatewayCallsConfig::default())
    }

    pub fn open(
        path: &std::path::Path,
        config: &GatewayCallsConfig,
    ) -> rusqlite::Result<Arc<Self>> {
        let (history, mut saved) = history::GatewayCallHistory::open(path, config)?;
        saved.sort_by_key(|record| record.seq);
        let mut latest = saved.last().map_or(0, |record| record.seq);
        let mut repaired = Vec::new();
        for record in &mut saved {
            if !matches!(
                record.phase,
                GatewayCallPhase::Routing | GatewayCallPhase::CallingUpstream
            ) {
                continue;
            }
            latest += 1;
            record.seq = latest;
            record.phase = GatewayCallPhase::Failed;
            record.success = false;
            record.error = Some("Gateway restarted before this call completed".to_string());
            record.duration_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .saturating_sub(record.at_unix_ms as u128) as u64;
            if config.enabled && config.persist {
                repaired.push(record.clone());
            }
        }
        let log = Self::from_saved(saved, Some(history), config.clone());
        for record in repaired {
            if let Some(history) = &log.history {
                history.save(record);
            }
        }
        Ok(log)
    }

    fn from_saved(
        mut saved: Vec<GatewayCallRecord>,
        history: Option<Arc<history::GatewayCallHistory>>,
        config: GatewayCallsConfig,
    ) -> Arc<Self> {
        saved.sort_by_key(|record| record.seq);
        let latest = saved.last().map_or(0, |record| record.seq);
        let records = saved
            .into_iter()
            .rev()
            .take(config.memory_capacity)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let (changed, _) = tokio::sync::watch::channel(latest);
        Arc::new(Self {
            records: std::sync::Mutex::new(records),
            latest: std::sync::atomic::AtomicU64::new(latest),
            history,
            config,
            changed,
        })
    }

    /// Compatibility helper for callers that only have a completed result.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        caller: Option<&str>,
        kind: GatewayCallKind,
        name: &str,
        instance_id: InstanceId,
        service_name: &str,
        duration: std::time::Duration,
        success: bool,
    ) {
        let mut record = self.new_record(caller, kind, name, Some(instance_id), service_name);
        record.phase = if success {
            GatewayCallPhase::Succeeded
        } else {
            GatewayCallPhase::Failed
        };
        record.duration_ms = duration.as_millis() as u64;
        record.success = success;
        self.update(record);
    }

    fn new_record(
        &self,
        caller: Option<&str>,
        kind: GatewayCallKind,
        name: &str,
        instance_id: Option<InstanceId>,
        service_name: &str,
    ) -> GatewayCallRecord {
        GatewayCallRecord {
            seq: 0,
            call_id: new_gateway_call_id(),
            phase: GatewayCallPhase::Routing,
            upstream_name: None,
            namespace_applied: false,
            name_mapped: false,
            error: None,
            routing_ms: None,
            request_write_ms: None,
            first_byte_ms: None,
            response_read_ms: None,
            at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            caller: caller.map(str::to_string),
            kind,
            name: name.to_string(),
            instance_id,
            service_name: service_name.to_string(),
            duration_ms: 0,
            success: false,
        }
    }

    fn begin(
        self: &Arc<Self>,
        caller: Option<&str>,
        kind: GatewayCallKind,
        name: &str,
        instance_id: Option<InstanceId>,
        service_name: &str,
        frame_call_id: Option<&str>,
    ) -> PendingGatewayCall {
        let mut record = self.new_record(caller, kind, name, instance_id, service_name);
        if let Some(call_id) = frame_call_id {
            record.call_id = call_id.to_string();
        }
        let trace = mcpstore::telemetry::CallTrace::register(record.call_id.clone());
        let mut pending = PendingGatewayCall {
            log: Arc::clone(self),
            record,
            trace,
            started: std::time::Instant::now(),
            finished: false,
        };
        pending.publish();
        pending
    }

    fn update(&self, mut record: GatewayCallRecord) {
        if !self.config.enabled {
            return;
        }
        let seq;
        {
            let mut records = self.records.lock().unwrap();
            seq = self.latest.load(std::sync::atomic::Ordering::Relaxed) + 1;
            record.seq = seq;
            if let Some(index) = records
                .iter()
                .position(|current| current.call_id == record.call_id)
            {
                records.remove(index);
            }
            while records.len() >= self.config.memory_capacity {
                records.pop_front();
            }
            records.push_back(record.clone());
            self.latest.store(seq, std::sync::atomic::Ordering::Relaxed);
        }
        if self.config.persist {
            if let Some(history) = &self.history {
                history.save(record);
            }
        }
        let _ = self.changed.send(seq);
    }

    pub fn latest_seq(&self) -> u64 {
        self.latest.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// Latest state per call with `seq > after`, oldest first.
    pub fn snapshot_after(&self, after: u64) -> Vec<GatewayCallRecord> {
        self.snapshot_page(after).1
    }

    /// Atomically pair the long-poll cursor with the records it represents.
    pub fn snapshot_page(&self, after: u64) -> (u64, Vec<GatewayCallRecord>) {
        let records = self.records.lock().unwrap();
        let latest = self.latest.load(std::sync::atomic::Ordering::Relaxed);
        let page = records
            .iter()
            .filter(|record| record.seq > after)
            .cloned()
            .collect();
        (latest, page)
    }
}

/// Publishes a failed terminal snapshot if the handler future is cancelled or
/// returns early. Successful completion disarms the guard exactly once.
struct PendingGatewayCall {
    log: Arc<GatewayCallLog>,
    record: GatewayCallRecord,
    trace: Arc<mcpstore::telemetry::CallTrace>,
    started: std::time::Instant,
    finished: bool,
}

impl PendingGatewayCall {
    fn publish(&self) {
        self.log.update(self.record.clone());
    }

    fn set_phase(&mut self, phase: GatewayCallPhase) {
        self.record.phase = phase;
        self.publish();
    }

    fn finish(mut self, success: bool, error: Option<String>) {
        self.complete(success, error);
    }

    fn complete(&mut self, success: bool, error: Option<String>) {
        if self.finished {
            return;
        }
        let timing = self.trace.timing();
        self.record.request_write_ms = timing.request_write_ms;
        self.record.first_byte_ms = timing.first_byte_ms;
        self.record.response_read_ms = timing.response_read_ms;
        self.record.duration_ms = self.started.elapsed().as_millis() as u64;
        self.record.phase = if success {
            GatewayCallPhase::Succeeded
        } else {
            GatewayCallPhase::Failed
        };
        self.record.success = success;
        self.record.error = error;
        self.finished = true;
        self.trace.unregister();
        self.log.update(self.record.clone());
    }
}

impl Drop for PendingGatewayCall {
    fn drop(&mut self) {
        if !self.finished {
            self.complete(
                false,
                Some("Call ended before a terminal response was recorded".to_string()),
            );
        }
    }
}

impl GatewayOptions {
    pub fn launch_descriptor(&self) -> GatewayLaunchDescriptor {
        GatewayLaunchDescriptor {
            url: format!("http://{}:{}{}", self.host, self.port, self.path),
        }
    }
}

const SESSION_STATE_LIST_TOOL: &str = "mcpstore_session_state_list";
const SESSION_STATE_GET_TOOL: &str = "mcpstore_session_state_get";
const SESSION_STATE_SET_TOOL: &str = "mcpstore_session_state_set";
const SESSION_STATE_DELETE_TOOL: &str = "mcpstore_session_state_delete";
const SESSION_STATE_CLEAR_TOOL: &str = "mcpstore_session_state_clear";
const SESSION_SNAPSHOT_EXPORT_TOOL: &str = "mcpstore_session_snapshot_export";
const SESSION_SNAPSHOT_IMPORT_TOOL: &str = "mcpstore_session_snapshot_import";
const TOOL_OVERRIDE_LIST_TOOL: &str = "mcpstore_tool_override_list";
const TOOL_OVERRIDE_GET_TOOL: &str = "mcpstore_tool_override_get";
const TOOL_OVERRIDE_SET_TOOL: &str = "mcpstore_tool_override_set";
const TOOL_OVERRIDE_DELETE_TOOL: &str = "mcpstore_tool_override_delete";
const PROMPT_OVERRIDE_LIST_TOOL: &str = "mcpstore_prompt_override_list";
const PROMPT_OVERRIDE_GET_TOOL: &str = "mcpstore_prompt_override_get";
const PROMPT_OVERRIDE_SET_TOOL: &str = "mcpstore_prompt_override_set";
const PROMPT_OVERRIDE_DELETE_TOOL: &str = "mcpstore_prompt_override_delete";
const RESOURCE_OVERRIDE_LIST_TOOL: &str = "mcpstore_resource_override_list";
const RESOURCE_OVERRIDE_GET_TOOL: &str = "mcpstore_resource_override_get";
const RESOURCE_OVERRIDE_SET_TOOL: &str = "mcpstore_resource_override_set";
const RESOURCE_OVERRIDE_DELETE_TOOL: &str = "mcpstore_resource_override_delete";
const RESOURCE_TEMPLATE_OVERRIDE_LIST_TOOL: &str = "mcpstore_resource_template_override_list";
const RESOURCE_TEMPLATE_OVERRIDE_GET_TOOL: &str = "mcpstore_resource_template_override_get";
const RESOURCE_TEMPLATE_OVERRIDE_SET_TOOL: &str = "mcpstore_resource_template_override_set";
const RESOURCE_TEMPLATE_OVERRIDE_DELETE_TOOL: &str = "mcpstore_resource_template_override_delete";
const OPENAPI_IMPORT_LIST_TOOL: &str = "mcpstore_openapi_import_list";
const OPENAPI_IMPORT_GET_TOOL: &str = "mcpstore_openapi_import_get";
const OPENAPI_IMPORT_SET_TOOL: &str = "mcpstore_openapi_import";
const OPENAPI_BUNDLE_TOOL: &str = "mcpstore_openapi_bundle";
const OPENAPI_BUNDLE_ARTIFACT_TOOL: &str = "mcpstore_openapi_bundle_artifact";
const SERVICE_LIST_TOOL: &str = "mcpstore_scope_instance_list";
const SERVICE_INFO_TOOL: &str = "mcpstore_service_scope_info";
const SERVICE_STATE_TOOL: &str = "mcpstore_service_state";
const SERVICE_CHECK_TOOL: &str = "mcpstore_instance_check";
const SERVICE_ADD_TOOL: &str = "mcpstore_service_definition_add";
const SERVICE_PATCH_TOOL: &str = "mcpstore_service_scope_set";
const SERVICE_REMOVE_TOOL: &str = "mcpstore_service_scope_remove";
const SERVICE_CONNECT_TOOL: &str = "mcpstore_instance_connect";
const SERVICE_DISCONNECT_TOOL: &str = "mcpstore_instance_disconnect";
const SERVICE_RESTART_TOOL: &str = "mcpstore_instance_restart";
const SERVICE_WAIT_TOOL: &str = "mcpstore_instance_wait";
const KV_HEALTH_TOOL: &str = "mcpstore_kv_health";
const KV_INSPECT_TOOL: &str = "mcpstore_kv_inspect";
const KV_SWITCH_TOOL: &str = "mcpstore_kv_switch";
const SEARCH_TOOL: &str = "mcpstore_search_tools";

#[derive(Clone)]
struct ToolBinding {
    tool: Tool,
    instance_id: InstanceId,
    tool_name: String,
    service_name: String,
    namespace_applied: bool,
    name_mapped: bool,
}

struct GatewayToolsChangedNotification {
    sink: SubscriptionSink,
    active: Arc<AtomicBool>,
    instance_id: Option<InstanceId>,
}

#[async_trait::async_trait]
impl EventHandler for GatewayToolsChangedNotification {
    async fn handle(&self, event: &Event) {
        if event.event_type != mcpstore::events::types::EventKind::McpToolsChanged.as_str() {
            return;
        }
        if let Some(instance_id) = self.instance_id {
            // A payload without a parseable instanceId is a broadcast (bridge
            // resubscription, cross-collection changes) — every mount consumes
            // it; only a valid, different id is filtered out.
            if let Some(changed_instance) = event
                .payload
                .get("instanceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<InstanceId>().ok())
            {
                if changed_instance != instance_id {
                    return;
                }
            }
        }
        if let Err(error) = self.sink.notify_tool_list_changed().await {
            tracing::debug!(%error, "aggregate MCP subscription closed before tools/list_changed");
        }
    }

    fn is_alive(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

/// Mount-lifetime catalog invalidator: registered once per mount (unlike the
/// per-subscription notification forwarder above). Local `McpToolsChanged`
/// events (tool syncs, override writes) mark the snapshot dirty and warm the
/// rebuild; the next request — or this warm task — swaps in fresh state.
/// Holds the catalog weakly: once the mount is unmounted and all server
/// clones drop, the bus's retain_alive reaps the handler on its next publish.
#[derive(Debug)]
struct CatalogInvalidator {
    catalog: std::sync::Weak<GatewayCatalog>,
}

#[async_trait::async_trait]
impl EventHandler for CatalogInvalidator {
    async fn handle(&self, event: &Event) {
        if event.event_type != mcpstore::events::types::EventKind::McpToolsChanged.as_str() {
            return;
        }
        let Some(catalog) = self.catalog.upgrade() else {
            return;
        };
        if let Some(instance_id) = catalog.instance_id {
            // Same broadcast rule as the notification forwarder: a payload
            // without a parseable instanceId invalidates every mount.
            if let Some(changed_instance) = event
                .payload
                .get("instanceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<InstanceId>().ok())
            {
                if changed_instance != instance_id {
                    return;
                }
            }
        }
        catalog.invalidate();
        tokio::spawn(async move {
            let _ = catalog.ensure().await;
        });
    }

    fn is_alive(&self) -> bool {
        self.catalog.strong_count() > 0
    }
}

/// One projection defect found while building a gateway mount's tool surface.
/// Upstream data defects are isolated to their owner: the mount always
/// builds, defects surface here (panel) and at call time (structured
/// errors) — never as a broken tool list.
#[derive(Clone, Debug, serde::Serialize)]
pub(crate) enum ProjectionIssue {
    /// Service failed to connect at mount build; its tools are excluded.
    Down { service: String, reason: String },
    /// Tool payload malformed; the tool is skipped.
    SkippedTool {
        service: Option<String>,
        tool: Option<String>,
        reason: String,
    },
    /// `input_schema` missing or not an object; exposed with an empty-object
    /// fallback schema.
    DegradedTool { service: String, tool: String },
}

#[derive(Clone)]
pub(crate) struct GatewayServer {
    store: Arc<MCPStore>,
    scope: ScopeRef,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    scope_label: String,
    /// Shared catalog: every server clone (one per streamable-http request
    /// factory call) reads and rebuilds through this one Arc.
    catalog: Arc<GatewayCatalog>,
    management: Arc<GatewayManagementTools>,
    /// Caller identity for this mount (`/mcp/<id>` segment); None = generic `/mcp`.
    caller: Option<String>,
    call_log: Arc<GatewayCallLog>,
}

/// Management tool families a mount can expose. Immutable per mount; one Arc
/// shared by the server dispatch and the catalog merge.
#[derive(Default)]
pub(crate) struct GatewayManagementTools {
    session_state_tools: Arc<HashMap<String, Tool>>,
    tool_override_tools: Arc<HashMap<String, Tool>>,
    prompt_override_tools: Arc<HashMap<String, Tool>>,
    resource_override_tools: Arc<HashMap<String, Tool>>,
    openapi_tools: Arc<HashMap<String, Tool>>,
    service_tools: Arc<HashMap<String, Tool>>,
    kv_tools: Arc<HashMap<String, Tool>>,
    search_tools: Arc<HashMap<String, Tool>>,
}

/// One served catalog state: bindings for the call path, merged+sorted tools
/// for tools/list.
#[derive(Clone)]
pub(crate) struct CatalogSnapshot {
    bindings: HashMap<String, ToolBinding>,
    tools: Vec<Tool>,
}

/// The tool catalog of one mount. The snapshot is built at mount build and
/// swapped by single-flight rebuilds; invalidation comes from local
/// `McpToolsChanged` events (tool syncs, override writes) and from a
/// call-path lookup miss (detailed in the handler). Readers always clone the
/// current Arc — a rebuild in flight never blocks them.
pub(crate) struct GatewayCatalog {
    // Rebuild inputs, frozen per mount.
    store: Arc<MCPStore>,
    scope: ScopeRef,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    management: Arc<GatewayManagementTools>,
    /// Projection report, refreshed on every successful rebuild.
    projection: Arc<std::sync::Mutex<Vec<ProjectionIssue>>>,
    /// Current snapshot.
    snapshot: std::sync::RwLock<Arc<CatalogSnapshot>>,
    /// Set by invalidation; cleared once a rebuild attempt completes
    /// successfully (a failed attempt keeps it so the next caller retries).
    dirty: AtomicBool,
    /// Single-flight rebuild gate: concurrent refreshers queue on one rebuild
    /// instead of stampeding the upstreams.
    rebuild_gate: tokio::sync::Mutex<()>,
    /// Bumped on every invalidate. A rebuild captures it at start and may
    /// only clear `dirty` if it still matches — otherwise an invalidation
    /// that raced an in-flight rebuild would be lost along with the stale
    /// snapshot it was meant to replace.
    generation: AtomicU64,
    /// Completed rebuilds since mount build (health observability).
    rebuilds: AtomicU64,
}

/// Merge downstream bindings with the mount's management tool families into
/// the served tools list. A management tool colliding with a downstream name
/// cannot build the surface — the same rule at mount build and at rebuild.
fn merged_tools(
    bindings: &HashMap<String, ToolBinding>,
    management: &GatewayManagementTools,
) -> Result<Vec<Tool>, String> {
    let families = [
        &management.session_state_tools,
        &management.tool_override_tools,
        &management.prompt_override_tools,
        &management.resource_override_tools,
        &management.openapi_tools,
        &management.service_tools,
        &management.kv_tools,
        &management.search_tools,
    ];
    for family in &families {
        for tool_name in family.keys() {
            if bindings.contains_key(tool_name) {
                return Err(format!(
                    "MCPStore management tool collides with a downstream tool, cannot build the gateway MCP surface: tool={tool_name}"
                ));
            }
        }
    }
    let mut tools: Vec<Tool> = bindings
        .values()
        .map(|binding| binding.tool.clone())
        .collect();
    for family in &families {
        tools.extend(family.values().cloned());
    }
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(tools)
}

impl GatewayCatalog {
    #[allow(clippy::too_many_arguments)]
    fn new(
        store: Arc<MCPStore>,
        scope: ScopeRef,
        instance_id: Option<InstanceId>,
        session_key: Option<String>,
        management: Arc<GatewayManagementTools>,
        snapshot: CatalogSnapshot,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            scope,
            instance_id,
            session_key,
            management,
            projection: Arc::new(std::sync::Mutex::new(Vec::new())),
            snapshot: std::sync::RwLock::new(Arc::new(snapshot)),
            dirty: AtomicBool::new(false),
            rebuild_gate: tokio::sync::Mutex::const_new(()),
            generation: AtomicU64::new(0),
            rebuilds: AtomicU64::new(0),
        })
    }

    /// Blank catalog for tests: nothing downstream, nothing merged.
    #[cfg(test)]
    pub(crate) fn blank(
        store: Arc<MCPStore>,
        scope: ScopeRef,
        instance_id: Option<InstanceId>,
        session_key: Option<String>,
    ) -> Arc<Self> {
        Self::new(
            store,
            scope,
            instance_id,
            session_key,
            Arc::new(GatewayManagementTools::default()),
            CatalogSnapshot {
                bindings: HashMap::new(),
                tools: Vec::new(),
            },
        )
    }

    pub(crate) fn current(&self) -> Arc<CatalogSnapshot> {
        Arc::clone(&self.snapshot.read().unwrap())
    }

    pub(crate) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::Release);
        self.dirty.store(true, Ordering::Release);
    }

    pub(crate) fn rebuild_count(&self) -> u64 {
        self.rebuilds.load(Ordering::Relaxed)
    }

    /// Serve the snapshot, rebuilding single-flight when invalidated.
    /// Stale-while-revalidate: callers arriving during a rebuild get the
    /// last-good snapshot. A failed rebuild keeps the dirty flag (the next
    /// caller retries) but keeps serving — an upstream being down must not
    /// break the whole surface.
    pub(crate) async fn ensure(&self) -> Arc<CatalogSnapshot> {
        loop {
            let snapshot = self.current();
            if !self.dirty.load(Ordering::Acquire) {
                return snapshot;
            }
            let _gate = self.rebuild_gate.lock().await;
            if self.dirty.load(Ordering::Acquire) {
                let generation = self.generation.load(Ordering::Acquire);
                match self.rebuild().await {
                    Ok(()) => {
                        // An invalidate that raced this rebuild bumped the
                        // generation: its dirty flag must survive so the
                        // next pass rebuilds against the new state.
                        if self.generation.load(Ordering::Acquire) == generation {
                            self.dirty.store(false, Ordering::Release);
                        }
                        self.rebuilds.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            "gateway catalog rebuild failed; serving last-good snapshot"
                        );
                    }
                }
            }
        }
    }

    /// Rebuild and swap the snapshot. The caller holds the rebuild gate.
    async fn rebuild(&self) -> Result<(), BoxErr> {
        let (bindings, issues) = transport::build_tool_bindings(
            &self.store,
            &self.scope,
            self.instance_id,
            self.session_key.as_deref(),
        )
        .await?;
        let tools = merged_tools(&bindings, &self.management)?;
        if let Ok(mut projection) = self.projection.lock() {
            *projection = issues;
        }
        *self.snapshot.write().unwrap() = Arc::new(CatalogSnapshot { bindings, tools });
        Ok(())
    }

    /// Live bindings for a session key that differs from the mount's own — a
    /// call naming its own business session key sees a different tool surface
    /// than the mount snapshot. Session tool lists are dynamic by contract,
    /// so this never serves from the snapshot.
    async fn fresh_bindings(
        &self,
        session_key: Option<&str>,
    ) -> Result<HashMap<String, ToolBinding>, BoxErr> {
        let (bindings, _issues) =
            transport::build_tool_bindings(&self.store, &self.scope, self.instance_id, session_key)
                .await?;
        Ok(bindings)
    }
}

mod catalog;
pub(crate) mod guards;
mod handler;
mod history;
mod search;
mod tests;
mod tools;
mod transport;

pub(crate) use transport::{normalize_http_path, streamable_http_router};
