//! REST client for the mcpstore backend. Blocking (ureq) — call from
//! `cx.background_spawn`; the 5s polling loop matches the old web UI.
//!
//! Two faces (memory: mcpstore-port-faces): `core` (:1820) serves service
//! management without prefix; `app` (:1821) serves /v1/meta, /v1/settings.

use anyhow::{anyhow, bail, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
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

    pub async fn list_tools(&self, addr: &ServiceAddress) -> Result<Vec<ToolInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}&filter=available", addr.base_path(), "/tools/list",),
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
    pub async fn add_service(
        &self,
        name: &str,
        transport: &str,
        target: &str,
        description: Option<&str>,
    ) -> Result<Value> {
        let mut payload = build_service_config(transport, target);
        payload["description"] = description.unwrap_or("").into();
        payload["_mcpstore"] = serde_json::json!({ "scopes": { "store": {} } });
        self.request(
            "POST",
            &format!("/services/{}", urlencoding::encode(name)),
            Some(&payload),
        ).await
    }

    /// `PUT /services/{name}/scopes/…` — 更新某作用域上的服务声明（编辑）。
    pub async fn update_service_scope(
        &self,
        name: &str,
        scope: &ScopeRef,
        transport: &str,
        target: &str,
        description: Option<&str>,
    ) -> Result<Value> {
        let mut config = build_service_config(transport, target);
        config["description"] = description.unwrap_or("").into();
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
        transport: &str,
        target: &str,
    ) -> Result<Value> {
        let config = build_service_config(transport, target);
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

    pub async fn show_config(&self, scope: &ScopeRef) -> Result<Value> {
        match scope {
            // agent config 在 core 面；store config 是 app 自有接口。
            ScopeRef::Agent { .. } => self.request("GET", &config_path(scope, None), None).await,
            ScopeRef::Store => self.request_app("GET", &config_path(scope, None), None).await,
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

    // mcp-hub / client-config 是 app 自有接口（附录C），固定走 app 面。
    pub async fn mcp_hub_status(&self, query: &str) -> Result<Value> {
        self.request_app("GET", &format!("/mcp-hub/status{query}"), None).await
    }

    pub async fn mcp_hub_action(&self, action: &str, query: &str) -> Result<Value> {
        self.request_app("POST", &format!("/mcp-hub/{action}{query}"), None).await
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
fn build_service_config(transport: &str, target: &str) -> Value {
    let mut config = serde_json::json!({
        "transport": transport,
        "env": {},
        "headers": {},
    });
    if transport == "stdio" {
        let parts: Vec<&str> = target.split_whitespace().collect();
        config["command"] = parts.first().copied().unwrap_or("").into();
        config["args"] = serde_json::to_value(parts.iter().skip(1).collect::<Vec<_>>())
            .unwrap_or(Value::Array(vec![]));
    } else {
        config["url"] = target.into();
    }
    config
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
    fn build_service_config_stdio_splits_command() {
        let config = build_service_config("stdio", "uvx mcp-server-fs /tmp");
        assert_eq!(config["command"], "uvx");
        assert_eq!(config["args"], serde_json::json!(["mcp-server-fs", "/tmp"]));
        let config = build_service_config("streamable-http", "http://x/y");
        assert_eq!(config["url"], "http://x/y");
    }
}
