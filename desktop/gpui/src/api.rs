//! REST client for the mcpstore backend. Blocking (ureq) — call from
//! `cx.background_spawn`; the 5s polling loop matches the old web UI.

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
            ScopeRef::Agent { agent_id } => format!(
                "?scope=agent&agent_id={}",
                urlencoding::encode(agent_id)
            ),
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
            ScopeView::Agent { agent_id } => format!(
                "?scope=agent&agent_id={}",
                urlencoding::encode(agent_id)
            ),
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
    pub added_time: f64,
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

/// mcpstore 客户端。`base` 形如 `http://127.0.0.1:1821/api`。
pub struct Api {
    agent: ureq::Agent,
    base: String,
}

impl Api {
    pub fn new(base: impl Into<String>) -> Self {
        // ponytail: single global timeout; per-request timeouts if any endpoint hangs
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            // 4xx/5xx arrive as Ok(Response); we parse the error body ourselves.
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.new_agent(),
            base: base.into(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let send = || -> std::result::Result<(u16, String), String> {
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
                    Ok((code, text))
                }
                Err(e) => Err(format!("{e}")),
            }
        };
        let (code, text) =
            send().map_err(|e| anyhow!("backend unreachable ({method} {path}): {e}"))?;
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

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let v = self.request("GET", path, None)?;
        Ok(serde_json::from_value(v)?)
    }

    /// Wired into the sidebar/status area in an upcoming milestone.
    #[allow(dead_code)]
    pub fn health(&self) -> Result<Value> {
        self.get("/health")
    }

    #[allow(dead_code)]
    pub fn meta(&self) -> Result<Value> {
        self.get("/v1/meta")
    }

    pub fn list_services(&self, view: &ScopeView) -> Result<Vec<ServiceInstance>> {
        let v = self.request("GET", &format!("/services/list{}", view.query()), None)?;
        Ok(serde_json::from_value(v["services"].clone())?)
    }

    pub fn list_scopes(&self) -> Result<Vec<ScopeSummary>> {
        let v = self.request("GET", "/scopes/list", None)?;
        Ok(serde_json::from_value(v["scopes"].clone())?)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentItem>> {
        let v = self.request("GET", "/agents/list", None)?;
        Ok(serde_json::from_value(v["agents"].clone())?)
    }

    pub fn get_service(&self, addr: &ServiceAddress) -> Result<ServiceInstance> {
        self.get(&addr.path())
    }

    pub fn list_tools(&self, addr: &ServiceAddress) -> Result<Vec<ToolInfo>> {
        let v = self.request(
            "GET",
            &format!("{}{}&filter=available", addr.base_path(), "/tools/list",),
            None,
        )?;
        Ok(serde_json::from_value(v["tools"].clone())?)
    }

    #[allow(dead_code)]
    pub fn call_tool(
        &self,
        addr: &ServiceAddress,
        tool_name: &str,
        args: Value,
    ) -> Result<Value> {
        self.request(
            "POST",
            &format!("{}{}", addr.base_path(), "/tools/call"),
            Some(&serde_json::json!({ "tool_name": tool_name, "args": args })),
        )
    }

    pub fn lifecycle(&self, addr: &ServiceAddress, action: &str) -> Result<Value> {
        self.request("POST", &format!("{}{}", addr.base_path(), action), None)
    }

    pub fn cache_health(&self) -> Result<Value> {
        self.get("/cache/health")
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
}
