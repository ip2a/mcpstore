//! REST client for the mcpstore backend. Blocking (ureq) — call from
//! `cx.background_spawn`; the 5s polling loop matches the old web UI.
//!
//! Two faces (memory: mcpstore-port-faces): `core` (:1820) serves service
//! management without prefix; `app` (:1821) serves /v1/meta, /v1/settings.

use anyhow::{anyhow, bail, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ScopeRef {
    Store,
    Agent { agent_id: String },
}

impl Default for ScopeRef {
    fn default() -> Self {
        Self::Store
    }
}

impl ScopeRef {
    /// 短标签（UI 展示用）。
    pub fn label(&self) -> String {
        match self {
            ScopeRef::Store => "store".into(),
            ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
        }
    }

    /// Query string for service-addressed endpoints (doc §17.2).
    pub fn query(&self) -> String {
        match self {
            ScopeRef::Store => "?scope=store".into(),
            ScopeRef::Agent { agent_id } => {
                format!("?scope=agent&agent_id={}", urlencoding::encode(agent_id))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ScopeView {
    Root,
    Store,
    Agent { agent_id: String },
}

impl ScopeView {
    pub fn query(&self) -> String {
        match self {
            ScopeView::Root => "?scope=root".into(),
            ScopeView::Store => "?scope=store".into(),
            ScopeView::Agent { agent_id } => {
                format!("?scope=agent&agent_id={}", urlencoding::encode(agent_id))
            }
        }
    }
}

/// 服务寻址：服务名 + 作用域（文档 §0.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAddress {
    pub service_name: String,
    pub scope: ScopeRef,
}

impl ServiceAddress {
    /// `/services/{name}`，无 query。
    fn base_path(&self) -> String {
        format!("/services/{}", urlencoding::encode(&self.service_name))
    }

    fn path(&self) -> String {
        format!("{}{}", self.base_path(), self.scope.query())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HealthMetrics {
    pub error_rate: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub latency_p99_ms: Option<f64>,
    pub sample_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecoveryState {
    Idle,
    Waiting {
        attempt: u32,
        retry_at: f64,
        hard_deadline: f64,
    },
    Probing {
        attempt: u32,
        hard_deadline: f64,
    },
    Exhausted {
        attempts: u32,
    },
}

impl Default for RecoveryState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ServiceAuthState {
    NotRequired,
    Unauthenticated,
    Authorizing,
    Authenticated,
    Refreshing,
    ScopeUpgradeRequired { required_scope: Option<String> },
    Failed,
}

impl Default for ServiceAuthState {
    fn default() -> Self {
        Self::NotRequired
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolsSyncState {
    pub status: String,
    pub items: Vec<ToolAvailabilityItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAvailabilityItem {
    pub name: String,
    pub availability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Readiness {
    pub status: String,
    pub reason: String,
    pub message: Option<String>,
    pub last_transition_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FailureInfo {
    pub phase: String,
    pub code: String,
    pub retryable: bool,
    pub message: String,
    pub since: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServiceState {
    pub instance_id: String,
    pub service_name: String,
    pub desired: String,
    pub phase: String,
    pub health: String,
    pub health_metrics: HealthMetrics,
    pub recovery: RecoveryState,
    pub auth: ServiceAuthState,
    pub tools: ToolsSyncState,
    pub readiness: Readiness,
    pub failure: Option<FailureInfo>,
    pub version: u64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolInfo {
    pub name: String,
    pub original_name: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Option<Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceTemplateInfo {
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PromptInfo {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServiceInstance {
    pub instance_id: String,
    pub service_name: String,
    pub scope: ScopeRef,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub state: ServiceState,
    #[serde(default)]
    pub tools: Vec<ToolInfo>,
    #[serde(default)]
    pub effective_config: Value,
    pub added_time: f64,
    pub mcp: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeSummary {
    pub scope: ScopeView,
    pub service_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentItem {
    pub agent_id: String,
    #[serde(default)]
    pub instance_ids: Vec<String>,
}

/// Auth status view (GET /services/:name/auth).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthStatusView {
    pub instance_id: String,
    pub status: String,
    pub flow: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub required_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthOperationResult {
    pub auth: Option<AuthStatusView>,
    pub authorization: Option<AuthorizationStart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthorizationStart {
    pub instance_id: String,
    pub authorization_url: String,
    pub scopes: Vec<String>,
}

/// Hosts config（settings.hosts）：多后端切换。决策 B：core 可切远程。
#[derive(Debug, Clone, Default)]
pub struct HostsConfig {
    pub active: String,
    pub entries: Vec<(String, String)>,
}

/// mcpstore 客户端。core 面如 `http://127.0.0.1:1820`，app 面如 `http://127.0.0.1:1821`。
/// 方法都是 async：native 下被 `background_spawn` 轮询在后台线程（内部阻塞 ureq），
/// wasm 下走浏览器 fetch（用 SendWrapper 满足 background_spawn 的 Send 约束）。
pub struct Api {
    #[cfg(not(target_arch = "wasm32"))]
    agent: ureq::Agent,
    core_base: std::sync::RwLock<String>,
    app_base: String,
}

impl Api {
    pub fn new(core_base: impl Into<String>, app_base: impl Into<String>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // ponytail: single global timeout; per-request timeouts if any endpoint hangs
            let config = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(10)))
                // 4xx/5xx arrive as Ok(Response); we parse the error body ourselves.
                .http_status_as_error(false)
                .build();
            Self {
                agent: config.new_agent(),
                core_base: std::sync::RwLock::new(core_base.into()),
                app_base: app_base.into(),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                core_base: std::sync::RwLock::new(core_base.into()),
                app_base: app_base.into(),
            }
        }
    }

    pub fn core_base(&self) -> String {
        self.core_base.read().unwrap().clone()
    }

    pub fn app_base(&self) -> &str {
        &self.app_base
    }

    /// 切换 core 后端（settings hosts 切换）。
    pub fn set_core_base(&self, url: impl Into<String>) {
        *self.core_base.write().unwrap() = url.into();
    }

    async fn request_at(
        &self,
        base: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let url = format!("{}{}", base, path);
        #[cfg(not(target_arch = "wasm32"))]
        let (code, text) = {
            let result = if let Some(b) = body {
                let rb = match method {
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.post(&url),
                };
                rb.send_json(b)
            } else {
                match method {
                    "PUT" => self.agent.put(&url).send_empty(),
                    "DELETE" => self.agent.delete(&url).call(),
                    "POST" => self.agent.post(&url).send_empty(),
                    _ => self.agent.get(&url).call(),
                }
            };
            match result {
                Ok(mut resp) => {
                    let code = resp.status().as_u16();
                    let text = resp.body_mut().read_to_string().unwrap_or_default();
                    (code, text)
                }
                Err(e) => return Err(anyhow!("backend unreachable ({method} {path}): {e}")),
            }
        };
        #[cfg(target_arch = "wasm32")]
        let (code, text) = {
            use gloo_net::http::Request;
            // 浏览器 fetch 链不是 Send；整条链包进一个 SendWrapper（单线程 web 上安全）。
            // ponytail: no request timeout on wasm — add AbortController if the daemon hangs.
            let fetch = async {
                let response = if let Some(b) = body {
                    let rb = match method {
                        "PUT" => Request::put(&url),
                        _ => Request::post(&url),
                    };
                    rb.json(b)?.send().await
                } else {
                    let request = match method {
                        "PUT" => Request::put(&url),
                        "DELETE" => Request::delete(&url),
                        "POST" => Request::post(&url),
                        _ => Request::get(&url),
                    };
                    request.send().await
                }?;
                let code = response.status();
                let text = response.text().await.unwrap_or_default();
                Ok::<_, gloo_net::Error>((code, text))
            };
            send_wrapper::SendWrapper::new(fetch)
                .await
                .map_err(|e| anyhow!("backend unreachable ({method} {path}): {e}"))?
        };
        let value: Value = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::Null)
        };
        if !(200..300).contains(&code) {
            bail!(error_message(code, &value, &text));
        }
        // Envelope: {success, data} / {ok, data}; plain bodies pass through.
        let enveloped = value.get("success").is_some() || value.get("ok").is_some();
        if enveloped {
            let ok = value
                .get("success")
                .or_else(|| value.get("ok"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if !ok {
                bail!(error_message(code, &value, &text));
            }
            return Ok(value.get("data").cloned().unwrap_or(Value::Null));
        }
        Ok(value)
    }

    async fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        let base = self.core_base.read().unwrap().clone();
        self.request_at(&base, method, path, body).await
    }

    async fn request_app(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        self.request_at(&self.app_base, method, path, body).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let v = self.request("GET", path, None).await?;
        Ok(serde_json::from_value(v)?)
    }

    // ---- app face (:1821) ----

    pub async fn meta(&self) -> Result<Value> {
        self.request_app("GET", "/v1/meta", None).await
    }

    pub async fn get_settings(&self) -> Result<Value> {
        self.request_app("GET", "/v1/settings", None).await
    }

    pub async fn update_settings(&self, payload: &Value) -> Result<Value> {
        self.request_app("PUT", "/v1/settings", Some(payload)).await
    }

    /// Probe an arbitrary core base: `GET {base}/health`; (latency ms, store name) when reachable.
    pub async fn probe_host(&self, base_url: &str) -> Option<(u64, Option<String>)> {
        let base = format!("{}/", base_url.trim_end_matches('/'));
        let start = std::time::Instant::now();
        let health = self.request_at(&base, "GET", "health", None).await.ok()?;
        let store = health
            .get("store")
            .and_then(|s| s.as_str())
            .map(str::to_string);
        Some((start.elapsed().as_millis() as u64, store))
    }

    // ---- core face: services ----

    pub async fn list_services(&self, view: &ScopeView) -> Result<Vec<ServiceInstance>> {
        let v = self.request("GET", &format!("/services/list{}", view.query()), None).await?;
        Ok(serde_json::from_value(v["services"].clone())?)
    }

    pub async fn list_scopes(&self) -> Result<Vec<ScopeSummary>> {
        let v = self.request("GET", "/scopes/list", None).await?;
        Ok(serde_json::from_value(v["scopes"].clone())?)
    }

    pub async fn list_agents(&self) -> Result<Vec<AgentItem>> {
        let v = self.request("GET", "/agents/list", None).await?;
        Ok(serde_json::from_value(v["agents"].clone())?)
    }

    pub async fn get_service(&self, addr: &ServiceAddress) -> Result<ServiceInstance> {
        self.get(&addr.path()).await
    }

    /// On-demand health probe: `GET /services/{name}/check`.
    pub async fn check_service(&self, addr: &ServiceAddress) -> Result<Value> {
        self.request("GET", &format!("{}/check", addr.base_path()), None).await
    }

    /// Upgrade auth scope: `POST /services/{name}/auth/scope-upgrade`.
    pub async fn auth_scope_upgrade(&self, addr: &ServiceAddress, required_scope: &str) -> Result<Value> {
        self.request(
            "POST",
            &format!("{}/auth/scope-upgrade", addr.base_path()),
            Some(&serde_json::json!({ "required_scope": required_scope })),
        ).await
    }

    /// `filter`: "available" (default) | "all" | "removed" — mirrors the web tools filter.
    pub async fn list_tools(&self, addr: &ServiceAddress, filter: &str) -> Result<Vec<ToolInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}&filter={}", addr.base_path(), "/tools/list", urlencoding::encode(filter)),
            None,
        ).await?;
        Ok(serde_json::from_value(v["tools"].clone())?)
    }

    pub async fn call_tool(
        &self,
        addr: &ServiceAddress,
        tool_name: &str,
        args: Value,
    ) -> Result<Value> {
        self.request(
            "POST",
            &format!("{}{}", addr.base_path(), "/tools/call"),
            Some(&serde_json::json!({ "tool_name": tool_name, "args": args })),
        ).await
    }

    pub async fn lifecycle(&self, addr: &ServiceAddress, action: &str) -> Result<Value> {
        self.request("POST", &format!("{}{}", addr.base_path(), action), None).await
    }

    /// `POST /services/{name}` — 新服务默认进 store 作用域（doc §17）。
    pub async fn add_service(&self, name: &str, draft: &ServiceFormDraft) -> Result<Value> {
        let mut payload = build_service_config(draft);
        payload["_mcpstore"] = mcpstore_extension(draft, serde_json::json!({ "store": {} }));
        self.request(
            "POST",
            &format!("/services/{}", urlencoding::encode(name)),
            Some(&payload),
        ).await
    }

    /// `PUT /services/{name}/scopes/…` — 更新某作用域上的服务声明（编辑）。
    /// Handshake/lifecycle are add-path (definition-level) fields and are not
    /// rewritten here: an omitted value would clear the existing scope state.
    pub async fn update_service_scope(
        &self,
        name: &str,
        scope: &ScopeRef,
        draft: &ServiceFormDraft,
    ) -> Result<Value> {
        let config = build_service_config(draft);
        let descriptor = serde_json::json!({ "config": config });
        self.request("PUT", &scope_descriptor_path(name, scope), Some(&descriptor)).await
    }

    /// `DELETE /services/{name}/scopes/…` — 从作用域移除声明（web 的“删除”）。
    pub async fn remove_service_scope(&self, name: &str, scope: &ScopeRef) -> Result<Value> {
        self.request("DELETE", &scope_descriptor_path(name, scope), None).await
    }

    /// `PUT /services/{name}/scopes/agents/{agent_id}` — 把服务声明进 agent 作用域。
    pub async fn declare_agent_scope(
        &self,
        name: &str,
        agent_id: &str,
        draft: &ServiceFormDraft,
    ) -> Result<Value> {
        let config = build_service_config(draft);
        self.request(
            "PUT",
            &format!(
                "/services/{}/scopes/agents/{}",
                urlencoding::encode(name),
                urlencoding::encode(agent_id)
            ),
            Some(&serde_json::json!({ "config": config })),
        ).await
    }

    /// `POST /scopes/agents/{agent_id}/reset`。
    pub async fn reset_agent_config(&self, agent_id: &str) -> Result<Value> {
        self.request(
            "POST",
            &format!("/scopes/agents/{}/reset", urlencoding::encode(agent_id)),
            None,
        ).await
    }

    // ---- tool policy ----

    pub async fn get_tool_policy(&self, addr: &ServiceAddress) -> Result<Value> {
        self.request("GET", &format!("{}{}", addr.base_path(), "/tool-policy"), None).await
    }

    pub async fn set_tool_policy(
        &self,
        addr: &ServiceAddress,
        available_tools: &[String],
    ) -> Result<Value> {
        self.request(
            "PUT",
            &format!("{}{}", addr.base_path(), "/tool-policy"),
            Some(&serde_json::json!({ "available_tools": available_tools })),
        ).await
    }

    pub async fn clear_tool_policy(&self, addr: &ServiceAddress) -> Result<Value> {
        self.request(
            "DELETE",
            &format!("{}{}", addr.base_path(), "/tool-policy"),
            None,
        ).await
    }

    // ---- resources / prompts ----

    pub async fn list_resources(&self, addr: &ServiceAddress) -> Result<Vec<ResourceInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}", addr.base_path(), "/resources/list"),
            None,
        ).await?;
        Ok(serde_json::from_value(v["resources"].clone())?)
    }

    pub async fn list_resource_templates(&self, addr: &ServiceAddress) -> Result<Vec<ResourceTemplateInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}", addr.base_path(), "/resources/templates"),
            None,
        ).await?;
        Ok(serde_json::from_value(v["resource_templates"].clone())?)
    }

    pub async fn list_prompts(&self, addr: &ServiceAddress) -> Result<Vec<PromptInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}", addr.base_path(), "/prompts/list"),
            None,
        ).await?;
        Ok(serde_json::from_value(v["prompts"].clone())?)
    }

    pub async fn read_resource(&self, addr: &ServiceAddress, uri: &str) -> Result<Value> {
        self.request(
            "GET",
            &format!(
                "{}{}&uri={}",
                addr.base_path(),
                "/resources/read",
                urlencoding::encode(uri)
            ),
            None,
        ).await
    }

    // ---- auth (OAuth) ----

    pub async fn auth_status(&self, addr: &ServiceAddress) -> Result<AuthStatusView> {
        let v = self.request("GET", &format!("{}{}", addr.base_path(), "/auth"), None).await?;
        Ok(serde_json::from_value(v["auth"].clone())?)
    }

    pub async fn auth_action(&self, addr: &ServiceAddress, action: &str) -> Result<AuthOperationResult> {
        let v = self.request("POST", &format!("{}{}", addr.base_path(), action), None).await?;
        Ok(serde_json::from_value(v)?)
    }

    // ---- overrides（core 面）----

    pub async fn list_overrides(&self, kind: &str) -> Result<Value> {
        self.request("GET", &format!("/{kind}_overrides"), None).await
    }

    pub async fn set_override(&self, kind: &str, name: &str, key: &str, patch: &Value) -> Result<Value> {
        self.request("PUT", &override_path(kind, name, key), Some(patch)).await
    }

    pub async fn delete_override(&self, kind: &str, name: &str, key: &str) -> Result<Value> {
        self.request("DELETE", &override_path(kind, name, key), None).await
    }

    // ---- cache / config：app 自有接口，固定走 app 面 ----

    pub async fn cache_health(&self) -> Result<Value> {
        self.request_app("GET", "/cache/health", None).await
    }

    pub async fn cache_inspect(&self) -> Result<Value> {
        self.request_app("GET", "/cache/inspect", None).await
    }

    pub async fn switch_cache(&self, backend: &str) -> Result<Value> {
        self.request_app(
            "POST",
            "/cache/switch",
            Some(&serde_json::json!({ "backend": backend })),
        ).await
    }

    pub async fn show_config(&self, scope: &ScopeRef, format: &str) -> Result<Value> {
        let path = format!("{}&format={}", config_path(scope, None), format);
        match scope {
            // agent config 在 core 面；store config 是 app 自有接口。
            ScopeRef::Agent { .. } => self.request("GET", &path, None).await,
            ScopeRef::Store => self.request_app("GET", &path, None).await,
        }
    }

    pub async fn reset_config(&self, scope: &ScopeRef) -> Result<Value> {
        match scope {
            ScopeRef::Agent { agent_id } => self.request(
                "POST",
                &format!("/scopes/agents/{}/reset", urlencoding::encode(agent_id)),
                None,
            ).await,
            ScopeRef::Store => self.request_app("POST", "/config/reset", None).await,
        }
    }

    // gateway / client-config 是 app 自有接口（附录C），固定走 app 面。
    pub async fn gateway_status(&self, query: &str) -> Result<Value> {
        self.request_app("GET", &format!("/gateway/status{query}"), None).await
    }

    pub async fn gateway_action(&self, action: &str, query: &str) -> Result<Value> {
        self.request_app("POST", &format!("/gateway/{action}{query}"), None).await
    }

    /// Recent routed tool calls; `after`+`wait_ms` long-poll (server caps the wait at 25s).
    pub async fn gateway_requests(&self, after: u64, wait_ms: u64) -> Result<Value> {
        self.request_app(
            "GET",
            &format!("/gateway/requests?after={after}&wait_ms={wait_ms}"),
            None,
        )
        .await
    }

    /// Clients wired to the gateway (GET /gateway/wired).
    pub async fn gateway_wired(&self) -> Result<Value> {
        self.request_app("GET", "/gateway/wired", None).await
    }

    /// Write the gateway URL into a client's MCP config (POST /gateway/wire).
    pub async fn gateway_wire(&self, client: &str, path: &str) -> Result<Value> {
        self.request_app(
            "POST",
            &format!(
                "/gateway/wire?client={}&path={}",
                urlencoding::encode(client),
                urlencoding::encode(path)
            ),
            None,
        )
        .await
    }

    /// Remove what wire wrote (POST /gateway/unwire); refuses hand-edited entries.
    pub async fn gateway_unwire(&self, client: &str) -> Result<Value> {
        self.request_app(
            "POST",
            &format!("/gateway/unwire?client={}", urlencoding::encode(client)),
            None,
        )
        .await
    }

    pub async fn import_client_services(&self, client: &str, path: &str, names: &[String]) -> Result<Value> {
        self.request_app(
            "POST",
            "/client-config/import",
            Some(&serde_json::json!({ "client": client, "path": path, "names": names })),
        ).await
    }
}

/// `{transport, command|url, env, headers}` — add/update 共用的服务配置体。
/// `KEY=VALUE` per line, blank lines ignored.
pub fn parse_kv_lines(text: &str) -> serde_json::Map<String, Value> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            (!key.is_empty()).then(|| (key.to_string(), Value::String(value.trim().to_string())))
        })
        .collect()
}

fn non_empty_lines(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// Service form draft: every field the panel can write for a service.
#[derive(Debug, Clone, Default)]
pub struct ServiceFormDraft {
    pub transport: String,
    pub command: String,
    pub args_text: String,
    pub url: String,
    pub description: String,
    pub working_dir: String,
    pub env_text: String,
    pub headers_text: String,
    pub oauth_client_id: String,
    pub oauth_client_metadata_url: String,
    pub handshake_mode: String,
    pub startup_policy: String,
    pub restart_policy: String,
}

const HANDSHAKE_MODES: [&str; 3] = ["auto", "initialize", "discover"];
const STARTUP_POLICIES: [&str; 3] = ["lazy", "manual", "on-store-start"];
const RESTART_POLICIES: [&str; 7] = [
    "no",
    "on-failure",
    "on-failure:1",
    "on-failure:3",
    "on-failure:5",
    "always",
    "unless-stopped",
];

impl ServiceFormDraft {
    /// Submit-time validation; empty handshake/policy strings mean backend defaults.
    pub fn validate(&self) -> Result<(), String> {
        if self.transport != "stdio" && self.transport != "streamable-http" {
            return Err("transport must be stdio or streamable-http".into());
        }
        if self.transport == "stdio" && self.command.trim().is_empty() {
            return Err("stdio config requires command".into());
        }
        if self.transport != "stdio" && self.url.trim().is_empty() {
            return Err("http config requires url".into());
        }
        let handshake = self.handshake_mode.trim();
        if !handshake.is_empty() && !HANDSHAKE_MODES.contains(&handshake) {
            return Err(format!("handshake mode must be one of: {}", HANDSHAKE_MODES.join(", ")));
        }
        let startup = self.startup_policy.trim();
        if !startup.is_empty() && !STARTUP_POLICIES.contains(&startup) {
            return Err(format!("startup policy must be one of: {}", STARTUP_POLICIES.join(", ")));
        }
        let restart = self.restart_policy.trim();
        if !restart.is_empty() && !RESTART_POLICIES.contains(&restart) {
            return Err(format!("restart policy must be one of: {}", RESTART_POLICIES.join(", ")));
        }
        Ok(())
    }

    /// Reverse a service config (from import or an existing instance) into a draft.
    pub fn from_config(config: &Value) -> Self {
        let mcpstore = config.get("_mcpstore");
        let lifecycle = mcpstore.and_then(|m| m.get("lifecycle"));
        let auth = config.get("auth");
        let oauth_enabled = auth
            .and_then(|a| a.get("type"))
            .and_then(Value::as_str)
            .is_some_and(|t| t == "oauth_authorization_code");
        let kv_text = |key: &str| {
            config
                .get(key)
                .and_then(Value::as_object)
                .map(|map| {
                    map.iter()
                        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default()
        };
        Self {
            transport: config
                .get("transport")
                .and_then(Value::as_str)
                .unwrap_or("stdio")
                .to_string(),
            command: config
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            args_text: config
                .get("args")
                .and_then(Value::as_array)
                .map(|args| {
                    args.iter()
                        .map(|v| v.as_str().unwrap_or(&v.to_string()).to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default(),
            url: config.get("url").and_then(Value::as_str).unwrap_or_default().to_string(),
            description: config
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            working_dir: config
                .get("workingDir")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            env_text: kv_text("env"),
            headers_text: kv_text("headers"),
            oauth_client_id: if oauth_enabled {
                auth.and_then(|a| a.get("client_id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            } else {
                String::new()
            },
            oauth_client_metadata_url: if oauth_enabled {
                auth.and_then(|a| a.get("client_metadata_url"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            } else {
                String::new()
            },
            handshake_mode: mcpstore
                .and_then(|m| m.get("handshake_mode"))
                .and_then(Value::as_str)
                .unwrap_or("auto")
                .to_string(),
            startup_policy: lifecycle
                .and_then(|l| l.get("startup_policy"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            restart_policy: lifecycle
                .and_then(|l| l.get("restart_policy"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }
    }
}

/// Parse a pasted JSON or TOML service config; accepts a bare config or a
/// single-entry `mcpServers` wrapper. Returns (name, config).
pub fn parse_imported_service_config(text: &str) -> Result<(Option<String>, Value), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("config text is empty".into());
    }
    let parsed = match serde_json::from_str::<Value>(text) {
        Ok(value) => value,
        Err(json_error) => {
            let toml_value: toml::Value = toml::from_str(text)
                .map_err(|toml_error| format!("not valid JSON or TOML: {json_error} / {toml_error}"))?;
            serde_json::to_value(toml_value).map_err(|e| e.to_string())?
        }
    };
    let Some(object) = parsed.as_object() else {
        return Err("config must be an object".into());
    };
    if let Some(Value::Object(servers)) = object.get("mcpServers") {
        if servers.len() != 1 {
            return Err("mcpServers must contain exactly one server".into());
        }
        let (name, config) = servers.iter().next().unwrap();
        if !config.is_object() {
            return Err("server config must be an object".into());
        }
        return Ok((Some(name.clone()), config.clone()));
    }
    Ok((None, parsed))
}

pub(crate) fn build_service_config(draft: &ServiceFormDraft) -> Value {
    let mut config = serde_json::json!({ "transport": draft.transport });
    let description = draft.description.trim();
    if !description.is_empty() {
        config["description"] = description.into();
    }
    if draft.transport == "stdio" {
        let command = draft.command.trim();
        if !command.is_empty() {
            config["command"] = command.into();
        }
        let args = non_empty_lines(&draft.args_text);
        if !args.is_empty() {
            config["args"] = Value::Array(args.into_iter().map(Value::String).collect());
        }
        let working_dir = draft.working_dir.trim();
        if !working_dir.is_empty() {
            config["workingDir"] = working_dir.into();
        }
        let env = parse_kv_lines(&draft.env_text);
        if !env.is_empty() {
            config["env"] = Value::Object(env);
        }
    } else {
        let url = draft.url.trim();
        if !url.is_empty() {
            config["url"] = url.into();
        }
        let headers = parse_kv_lines(&draft.headers_text);
        if !headers.is_empty() {
            config["headers"] = Value::Object(headers);
        }
        let client_id = draft.oauth_client_id.trim();
        let metadata_url = draft.oauth_client_metadata_url.trim();
        if !client_id.is_empty() || !metadata_url.is_empty() {
            let mut auth = serde_json::json!({ "type": "oauth_authorization_code" });
            if !client_id.is_empty() {
                auth["client_id"] = client_id.into();
            } else {
                auth["client_metadata_url"] = metadata_url.into();
            }
            config["auth"] = auth;
        }
    }
    config
}

/// `_mcpstore` extension: scope registry plus definition-level handshake/lifecycle.
/// Empty handshake ("auto") and empty policies are omitted so backend defaults apply.
fn mcpstore_extension(draft: &ServiceFormDraft, scopes: Value) -> Value {
    let mut extension = serde_json::json!({ "scopes": scopes });
    let handshake = draft.handshake_mode.trim();
    if !handshake.is_empty() && handshake != "auto" {
        extension["handshake_mode"] = handshake.into();
    }
    let startup = draft.startup_policy.trim();
    let restart = draft.restart_policy.trim();
    if !startup.is_empty() || !restart.is_empty() {
        let mut lifecycle = serde_json::json!({});
        if !startup.is_empty() {
            lifecycle["startup_policy"] = startup.into();
        }
        if !restart.is_empty() {
            lifecycle["restart_policy"] = restart.into();
        }
        extension["lifecycle"] = lifecycle;
    }
    extension
}

fn scope_descriptor_path(name: &str, scope: &ScopeRef) -> String {
    match scope {
        ScopeRef::Store => format!("/services/{}/scopes/store", urlencoding::encode(name)),
        ScopeRef::Agent { agent_id } => format!(
            "/services/{}/scopes/agents/{}",
            urlencoding::encode(name),
            urlencoding::encode(agent_id)
        ),
    }
}

fn config_path(scope: &ScopeRef, service_name: Option<&str>) -> String {
    let mut path = match scope {
        ScopeRef::Store => "/config?scope=store".to_string(),
        ScopeRef::Agent { agent_id } => format!(
            "/scopes/agents/{}/config?scope=agent",
            urlencoding::encode(agent_id)
        ),
    };
    if let Some(name) = service_name {
        path.push_str(&format!("&service_name={}", urlencoding::encode(name)));
    }
    path
}

fn override_path(kind: &str, name: &str, key: &str) -> String {
    let base = format!("/services/{}/{}_overrides", urlencoding::encode(name), kind);
    match kind {
        "resource" => format!("{base}?uri={}", urlencoding::encode(key)),
        "resource_template" => format!("{base}?uri_template={}", urlencoding::encode(key)),
        _ => format!("{base}/{}", urlencoding::encode(key)),
    }
}

fn error_message(code: u16, value: &Value, raw: &str) -> String {
    value
        .get("message")
        .and_then(|v| v.as_str())
        .or_else(|| {
            value
                .pointer("/errors/0/message")
                .and_then(|v| v.as_str())
        })
        .or_else(|| value.get("error").and_then(|v| v.as_str()))
        .map(|m| format!("HTTP {code}: {m}"))
        .unwrap_or_else(|| format!("HTTP {code}: {}", truncate(raw, 200)))
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_ref_query_roundtrip() {
        let store = ScopeRef::Store;
        assert_eq!(store.query(), "?scope=store");
        let agent = ScopeRef::Agent {
            agent_id: "a b".into(),
        };
        assert_eq!(agent.query(), "?scope=agent&agent_id=a%20b");
    }

    #[test]
    fn parse_service_instance_json() {
        let raw = serde_json::json!({
            "instance_id": "i1", "service_name": "fs", "scope": {"type": "store"},
            "transport": "stdio", "url": null, "command": "uvx",
            "state": {
                "instance_id": "i1", "service_name": "fs", "scope": {"type": "store"},
                "desired": "running", "phase": "running", "health": "healthy",
                "health_metrics": {"error_rate": null, "latency_p95_ms": 12, "latency_p99_ms": 30, "sample_size": 10},
                "recovery": {"status": "waiting", "attempt": 1, "retry_at": 1, "hard_deadline": 2},
                "auth": {"status": "authenticated"},
                "tools": {"status": "ready", "items": []},
                "readiness": {"status": "ready", "reason": "ready", "message": null, "last_transition_at": 0},
                "failure": null, "version": 1, "updated_at": 2
            },
            "tools": [], "added_time": 0
        });
        let svc: ServiceInstance = serde_json::from_value(raw).unwrap();
        assert_eq!(svc.service_name, "fs");
        assert_eq!(svc.state.phase, "running");
        assert!(matches!(svc.state.recovery, RecoveryState::Waiting { .. }));
    }

    #[test]
    fn service_form_draft_builds_stdio_and_http_configs() {
        let draft = ServiceFormDraft {
            transport: "stdio".into(),
            command: "uvx".into(),
            args_text: "mcp-server-fs\n/tmp".into(),
            env_text: "A=1\n\nB=two words".into(),
            ..Default::default()
        };
        draft.validate().unwrap();
        let config = build_service_config(&draft);
        assert_eq!(config["command"], "uvx");
        assert_eq!(config["args"], serde_json::json!(["mcp-server-fs", "/tmp"]));
        assert_eq!(config["env"]["A"], "1");
        assert_eq!(config["env"]["B"], "two words");
        assert!(config.get("workingDir").is_none());

        let draft = ServiceFormDraft {
            transport: "streamable-http".into(),
            url: "http://x/y".into(),
            headers_text: "Authorization=Bearer t".into(),
            oauth_client_metadata_url: "https://x/.well-known/oauth".into(),
            handshake_mode: "discover".into(),
            startup_policy: "lazy".into(),
            ..Default::default()
        };
        draft.validate().unwrap();
        let config = build_service_config(&draft);
        assert_eq!(config["url"], "http://x/y");
        assert_eq!(config["headers"]["Authorization"], "Bearer t");
        assert_eq!(config["auth"]["type"], "oauth_authorization_code");
        let ext = mcpstore_extension(&draft, serde_json::json!({ "store": {} }));
        assert_eq!(ext["handshake_mode"], "discover");
        assert_eq!(ext["lifecycle"]["startup_policy"], "lazy");
        assert!(ext["lifecycle"].get("restart_policy").is_none());
    }

    #[test]
    fn parse_imported_service_config_unwraps_mcp_servers() {
        let (name, config) = parse_imported_service_config(
            r#"{"mcpServers": {"fs": {"command": "uvx", "args": ["mcp-server-fs"]}}}"#,
        )
        .unwrap();
        assert_eq!(name.as_deref(), Some("fs"));
        assert_eq!(config["command"], "uvx");

        let (name, config) = parse_imported_service_config("command = 'npx'").unwrap();
        assert!(name.is_none());
        assert_eq!(config["command"], "npx");

        let draft = ServiceFormDraft::from_config(&config);
        assert_eq!(draft.transport, "stdio");
        assert_eq!(draft.command, "npx");

        assert!(
            parse_imported_service_config(r#"{"mcpServers": {}}"#).is_err()
        );
    }
}
