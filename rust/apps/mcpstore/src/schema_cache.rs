//! Disk cache for tool schemas, keyed by instance.
//!
//! Each `call` fetches the target tool's `input_schema` to validate arguments.
//! Without a cache that is an extra `list_tools` round-trip on every invocation.
//! This module persists the full tool list per instance with a TTL, so repeated
//! calls to the same instance skip the fetch entirely.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entries expire after this many seconds.
const TTL_SECS: u64 = 300;

fn cache_dir() -> PathBuf {
    mcpstore::ConfigManager::new()
        .mcp_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("schema-cache")
}

fn cache_path(instance_id: &str) -> PathBuf {
    cache_dir().join(format!("{instance_id}.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Load the cached tool list for an instance. Returns `None` on miss, parse
/// failure, or TTL expiry.
pub fn load(instance_id: &str) -> Option<Vec<Value>> {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(cache_path(instance_id)).ok()?).ok()?;
    let ts = data.get("ts")?.as_u64()?;
    if now_secs().saturating_sub(ts) > TTL_SECS {
        return None;
    }
    data.get("tools").and_then(Value::as_array).cloned()
}

/// Persist the full tool list for an instance. Failures are silently ignored —
/// the cache is a performance optimization, not a correctness requirement.
pub fn save(instance_id: &str, tools: &[Value]) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let data = serde_json::json!({ "ts": now_secs(), "tools": tools });
    let _ = std::fs::write(cache_path(instance_id), data.to_string());
}

/// Extract a single tool's `schema` field from a cached list.
pub fn find_schema(tools: &[Value], tool_name: &str) -> Option<Value> {
    tools
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(tool_name))
        .and_then(|t| t.get("schema").cloned())
}

// ── Target (name→instance) cache ──────────────────────────────────────────

fn target_cache_path() -> PathBuf {
    cache_dir().join("target-cache.json")
}

/// Load a cached name→instance mapping. Returns `None` on miss or TTL expiry.
pub fn load_target(scope: &str, name: &str) -> Option<String> {
    let key = format!("{scope}:{name}");
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(target_cache_path()).ok()?).ok()?;
    let ts = data.get("ts")?.as_u64()?;
    if now_secs().saturating_sub(ts) > TTL_SECS {
        return None;
    }
    data.get("targets")
        .and_then(|t| t.get(&key))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

/// Persist a name→instance mapping. Failures are silently ignored.
pub fn save_target(scope: &str, name: &str, instance_id: &str) {
    let key = format!("{scope}:{name}");
    let path = target_cache_path();
    let mut data: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({ "targets": {} }));
    data["ts"] = json!(now_secs());
    if let Some(targets) = data.get_mut("targets").and_then(Value::as_object_mut) {
        targets.insert(key, json!(instance_id));
    }
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(path, data.to_string());
}
