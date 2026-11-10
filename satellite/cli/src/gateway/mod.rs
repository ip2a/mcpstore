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
    config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig},
    events::{bus::EventHandler, Event},
    ContentItem, InstanceId, JsonStoreConfig, MCPStore, McpExecutionOptions, McpToolExecution,
    OpenApiBundleOptions, OpenApiImportOptions, OpenApiRefCachePolicy, PromptOverridePatch,
    ResourceOverridePatch, ResourceTemplateOverridePatch, ScopeRef, StoreOptions,
    ToolOverridePatch,
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
#[derive(Clone, Debug)]
pub struct GatewayOptions {
    pub config_path: Option<String>,
    pub store: Option<JsonStoreConfig>,
    pub namespace: Option<String>,
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
    pub expose_cache_tools: bool,
    pub expose_search_tools: bool,
}

impl Default for GatewayOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            store: None,
            namespace: None,
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
            expose_cache_tools: false,
            expose_search_tools: false,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct GatewayLaunchDescriptor {
    pub url: String,
}

/// What kind of business call a routed record represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCallKind {
    Tool,
    Prompt,
    Resource,
}

/// One routed business call through the gateway (management tools excluded).
#[derive(Clone, Debug, serde::Serialize)]
pub struct GatewayCallRecord {
    pub seq: u64,
    pub at_unix_ms: u64,
    /// Mount the call came through (`/mcp/<id>`); None = the generic `/mcp`.
    pub caller: Option<String>,
    pub kind: GatewayCallKind,
    /// Tool/prompt name, or the resolved upstream URI for resources.
    pub name: String,
    pub instance_id: InstanceId,
    pub service_name: String,
    pub duration_ms: u64,
    pub success: bool,
}

/// Bounded in-memory ring of recent routed calls; read by the app-plane /gateway/requests
/// with `?after=<seq>` long-polling (watch wakes the waiter on each push).
pub struct GatewayCallLog {
    records: std::sync::Mutex<std::collections::VecDeque<GatewayCallRecord>>,
    latest: std::sync::atomic::AtomicU64,
    changed: tokio::sync::watch::Sender<u64>,
}

const GATEWAY_CALL_LOG_CAP: usize = 256;

impl GatewayCallLog {
    pub fn new() -> Arc<Self> {
        let (changed, _) = tokio::sync::watch::channel(0);
        Arc::new(Self {
            records: std::sync::Mutex::new(std::collections::VecDeque::new()),
            latest: std::sync::atomic::AtomicU64::new(0),
            changed,
        })
    }

    /// Record one routed call with `now` timestamping, then wake waiters.
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
        self.push(GatewayCallRecord {
            seq: 0,
            at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default(),
            caller: caller.map(str::to_string),
            kind,
            name: name.to_string(),
            instance_id,
            service_name: service_name.to_string(),
            duration_ms: duration.as_millis() as u64,
            success,
        });
    }

    /// Push at the ring tail, dropping the oldest past the cap, and wake waiters.
    fn push(&self, mut record: GatewayCallRecord) {
        let seq;
        {
            let mut records = self.records.lock().unwrap();
            record.seq = self.latest.load(std::sync::atomic::Ordering::Relaxed) + 1;
            while records.len() >= GATEWAY_CALL_LOG_CAP {
                records.pop_front();
            }
            seq = record.seq;
            records.push_back(record);
        }
        self.latest.store(seq, std::sync::atomic::Ordering::Relaxed);
        let _ = self.changed.send(seq);
    }

    pub fn latest_seq(&self) -> u64 {
        self.latest.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// Records with `seq > after`, oldest first.
    pub fn snapshot_after(&self, after: u64) -> Vec<GatewayCallRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|record| record.seq > after)
            .cloned()
            .collect()
    }
}

impl GatewayOptions {
    pub fn launch_descriptor(&self) -> GatewayLaunchDescriptor {
        GatewayLaunchDescriptor {
            url: format!("http://{}:{}{}", self.host, self.port, self.path),
        }
    }

    pub fn to_store_options(&self) -> StoreOptions {
        StoreOptions {
            config_path: self.config_path.clone(),
            store: self.store.clone(),
            namespace: self.namespace.clone(),
            panel: mcpstore::PanelRole::ControlPanel,
        }
    }

    pub fn store_args(&self) -> crate::store_args::StoreSourceArgs {
        crate::store_args::StoreSourceArgs {
            control_panel: false,
            data_panel: false,
            config_path: self.config_path.clone(),
            // Unified model: --source no longer selects semantics; the rebuild uses CLI defaults
            source: crate::store_args::SourceArg::Local,
            store: self.store.as_ref().map(|store| store.store.clone()),
            store_config: self.store.as_ref().map(|store| store.config.to_string()),
            namespace: self.namespace.clone(),
            panel_id: None,
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
const CACHE_HEALTH_TOOL: &str = "mcpstore_cache_health";
const CACHE_INSPECT_TOOL: &str = "mcpstore_cache_inspect";
const CACHE_SWITCH_TOOL: &str = "mcpstore_cache_switch";
const SEARCH_TOOL: &str = "mcpstore_search_tools";

#[derive(Clone)]
struct ToolBinding {
    tool: Tool,
    instance_id: InstanceId,
    tool_name: String,
    service_name: String,
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
            let changed_instance = event
                .payload
                .get("instanceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<InstanceId>().ok());
            if changed_instance != Some(instance_id) {
                return;
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
            let changed_instance = event
                .payload
                .get("instanceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<InstanceId>().ok());
            if changed_instance != Some(instance_id) {
                return;
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
    cache_tools: Arc<HashMap<String, Tool>>,
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
    down: Arc<HashMap<InstanceId, transport::DownService>>,
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
        &management.cache_tools,
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
        down: Arc<HashMap<InstanceId, transport::DownService>>,
        management: Arc<GatewayManagementTools>,
        snapshot: CatalogSnapshot,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            scope,
            instance_id,
            session_key,
            down,
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
            Arc::new(HashMap::new()),
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
            &self.down,
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
        let (bindings, _issues) = transport::build_tool_bindings(
            &self.store,
            &self.scope,
            self.instance_id,
            session_key,
            &self.down,
        )
        .await?;
        Ok(bindings)
    }
}

mod catalog;
pub(crate) mod guards;
mod handler;
mod search;
mod tests;
mod tools;
mod transport;

pub(crate) use transport::streamable_http_router;
