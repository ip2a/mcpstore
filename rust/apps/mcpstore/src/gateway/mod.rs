//! Gateway: projects the store's services as one configurable MCP surface over
//! streamable-http (the only transport). What gets exposed and where it routes are setup
//! options, not MCP-serving boilerplate.

use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
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
        ServerCapabilities, ServerInfo, SubscriptionFilter, Tool, ToolAnnotations,
    },
    service::{SubscriptionContext, SubscriptionSink},
    transport::{
        streamable_http_server::session::local::LocalSessionManager,
        StreamableHttpServerConfig, StreamableHttpService,
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

/// One routed business tool call through the gateway (management tools excluded).
#[derive(Clone, Debug, serde::Serialize)]
pub struct GatewayCallRecord {
    pub at_unix_ms: u64,
    pub tool: String,
    pub instance_id: InstanceId,
    pub duration_ms: u64,
    pub success: bool,
}

/// Bounded in-memory ring of recent routed calls; read by the app-plane /gateway/requests.
pub type GatewayCallLog = Arc<std::sync::Mutex<std::collections::VecDeque<GatewayCallRecord>>>;

const GATEWAY_CALL_LOG_CAP: usize = 256;

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

#[derive(Clone)]
pub(crate) struct GatewayServer {
    store: Arc<MCPStore>,
    scope: ScopeRef,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    scope_label: String,
    bindings: Arc<HashMap<String, ToolBinding>>,
    session_state_tools: Arc<HashMap<String, Tool>>,
    tool_override_tools: Arc<HashMap<String, Tool>>,
    prompt_override_tools: Arc<HashMap<String, Tool>>,
    resource_override_tools: Arc<HashMap<String, Tool>>,
    openapi_tools: Arc<HashMap<String, Tool>>,
    service_tools: Arc<HashMap<String, Tool>>,
    cache_tools: Arc<HashMap<String, Tool>>,
    search_tools: Arc<HashMap<String, Tool>>,
    tools: Arc<Vec<Tool>>,
    call_log: GatewayCallLog,
}

mod catalog;
mod handler;
mod search;
mod tests;
mod tools;
mod transport;

pub(crate) use transport::{run_streamable_http, streamable_http_router};
