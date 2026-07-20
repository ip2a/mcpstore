//! APP-level diagnostics: bounded tool-call history and its storage policy.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::config::{HistoryConfig, HistoryPayload, HistoryStorage};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallHistoryRecord {
    pub id: String,
    pub timestamp: i64,
    pub instance_id: String,
    pub service_name: String,
    pub scope: String,
    pub tool_name: String,
    pub status: String,
    pub latency_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Diagnostics {
    config: Arc<RwLock<HistoryConfig>>,
    records: Arc<Mutex<VecDeque<ToolCallHistoryRecord>>>,
    path: Option<PathBuf>,
}

impl Diagnostics {
    pub fn new(config: HistoryConfig, app_path: &Path) -> Self {
        let path = Some(
            app_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("history")
                .join("tool-calls.jsonl"),
        );
        let mut records = VecDeque::new();
        if matches!(config.storage, HistoryStorage::Disk) {
            if let Some(path) = &path {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for line in content.lines() {
                        if let Ok(record) = serde_json::from_str(line) {
                            records.push_back(record);
                        }
                    }
                    purge_expired(&mut records, config.retention_days);
                    while records.len() > config.max_records.max(1) {
                        records.pop_front();
                    }
                }
            }
        }
        Self {
            config: Arc::new(RwLock::new(config)),
            records: Arc::new(Mutex::new(records)),
            path,
        }
    }

    pub async fn update_config(&self, config: HistoryConfig) {
        *self.config.write().await = config;
    }

    pub async fn config(&self) -> HistoryConfig {
        self.config.read().await.clone()
    }

    pub async fn record_tool_call(&self, mut record: ToolCallHistoryRecord) {
        let config = self.config.read().await.clone();
        if !config.enabled {
            return;
        }
        if !matches!(config.payload, HistoryPayload::Full) {
            record.arguments = None;
        }

        let mut records = self.records.lock().await;
        purge_expired(&mut records, config.retention_days);
        records.push_back(record);
        while records.len() > config.max_records.max(1) {
            records.pop_front();
        }

        // ponytail: rewrite the bounded JSONL file instead of adding a database
        // dependency; replace with SQLite when history queries become complex.
        if matches!(config.storage, HistoryStorage::Disk) {
            if let Some(path) = &self.path {
                if let Err(error) = write_jsonl(path, &records, config.max_size_bytes) {
                    tracing::warn!(%error, "failed to persist tool-call history");
                }
            }
        }
    }

    pub async fn recent_tool_calls(&self, count: usize) -> Vec<ToolCallHistoryRecord> {
        let records = self.records.lock().await;
        records.iter().rev().take(count).cloned().collect()
    }

    pub async fn clear_tool_calls(&self) -> std::io::Result<()> {
        self.records.lock().await.clear();
        if let Some(path) = &self.path {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

fn write_jsonl(
    path: &Path,
    records: &VecDeque<ToolCallHistoryRecord>,
    max_size_bytes: u64,
) -> std::io::Result<()> {
    let mut lines = Vec::new();
    let mut size = 0u64;
    for record in records.iter().rev() {
        let mut line = serde_json::to_vec(record).map_err(std::io::Error::other)?;
        line.push(b'\n');
        if size + line.len() as u64 > max_size_bytes.max(1) {
            break;
        }
        size += line.len() as u64;
        lines.push(line);
    }
    lines.reverse();
    let mut content = Vec::with_capacity(size as usize);
    for line in lines {
        content.extend_from_slice(&line);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

fn purge_expired(records: &mut VecDeque<ToolCallHistoryRecord>, retention_days: Option<u64>) {
    let Some(days) = retention_days else {
        return;
    };
    let cutoff = chrono::Utc::now().timestamp_millis() - (days.saturating_mul(86_400_000) as i64);
    records.retain(|record| record.timestamp >= cutoff);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn history_is_disabled_and_bounded_by_policy() {
        let root =
            std::env::temp_dir().join(format!("mcpstore-diagnostics-{}", uuid::Uuid::new_v4()));
        let mut config = HistoryConfig::default();
        config.max_records = 1;
        config.enabled = true;
        let diagnostics = Diagnostics::new(config, &root.join("app.toml"));

        for id in ["first", "second"] {
            diagnostics
                .record_tool_call(ToolCallHistoryRecord {
                    id: id.to_string(),
                    timestamp: 0,
                    instance_id: "instance".to_string(),
                    service_name: "service".to_string(),
                    scope: "store".to_string(),
                    tool_name: "tool".to_string(),
                    status: "success".to_string(),
                    latency_ms: 1.0,
                    arguments: None,
                    error: None,
                })
                .await;
        }

        let records = diagnostics.recent_tool_calls(10).await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "second");
        diagnostics.clear_tool_calls().await.unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
