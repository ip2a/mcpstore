use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::cache::CacheLayerManager;
use crate::identity::InstanceId;
use crate::transport::tasks::{McpTask, McpTaskStatus};

pub(crate) const TASK_ENTITY_TYPE: &str = "tasks";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpTaskRecord {
    pub instance_id: InstanceId,
    pub task_id: String,
    #[serde(default)]
    pub tool_name: Option<String>,
    pub task: McpTask,
    pub last_observed_at: String,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub(crate) struct TaskStateStore {
    cache: Arc<CacheLayerManager>,
}

impl TaskStateStore {
    pub(crate) fn new(cache: Arc<CacheLayerManager>) -> Self {
        Self { cache }
    }

    pub(crate) async fn observe(
        &self,
        instance_id: InstanceId,
        task: McpTask,
        tool_name: Option<&str>,
    ) -> crate::cache::Result<McpTaskRecord> {
        let key = task_key(instance_id, &task.task_id);
        let previous = self.get_by_key(&key).await?;
        let now = Utc::now();
        let record = McpTaskRecord {
            instance_id,
            task_id: task.task_id.clone(),
            tool_name: tool_name.map(str::to_string).or_else(|| {
                previous
                    .as_ref()
                    .and_then(|record| record.tool_name.clone())
            }),
            task,
            last_observed_at: now.to_rfc3339(),
            last_error: None,
        };
        self.put_record(&key, &record).await?;
        Ok(record)
    }

    pub(crate) async fn get(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> crate::cache::Result<Option<McpTaskRecord>> {
        let key = task_key(instance_id, task_id);
        self.get_by_key(&key).await
    }

    pub(crate) async fn list(
        &self,
        instance_id: InstanceId,
    ) -> crate::cache::Result<Vec<McpTaskRecord>> {
        let entities = self.cache.get_all_entities_async(TASK_ENTITY_TYPE).await?;
        let mut records = Vec::new();
        for (_key, value) in entities {
            let record: McpTaskRecord = serde_json::from_value(value)?;
            if record.instance_id != instance_id {
                continue;
            }
            records.push(record);
        }
        records.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        Ok(records)
    }

    pub(crate) async fn record_error(
        &self,
        instance_id: InstanceId,
        task_id: &str,
        error: impl Into<String>,
    ) -> crate::cache::Result<()> {
        let key = task_key(instance_id, task_id);
        let Some(value) = self.cache.get_entity(TASK_ENTITY_TYPE, &key).await? else {
            return Ok(());
        };
        let mut record: McpTaskRecord = serde_json::from_value(value)?;
        record.last_error = Some(error.into());
        self.put_record(&key, &record).await
    }

    pub(crate) async fn mark_disconnected(
        &self,
        instance_id: InstanceId,
        reason: impl Into<String>,
    ) -> crate::cache::Result<()> {
        let reason = reason.into();
        let records = self.list(instance_id).await?;
        for mut record in records {
            if record.task.status.is_terminal() {
                continue;
            }
            record.task.status = McpTaskStatus::Disconnected;
            record.last_error = Some(reason.clone());
            let key = task_key(instance_id, &record.task_id);
            self.put_record(&key, &record).await?;
        }
        Ok(())
    }

    async fn get_by_key(&self, key: &str) -> crate::cache::Result<Option<McpTaskRecord>> {
        self.cache
            .get_entity(TASK_ENTITY_TYPE, key)
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }

    async fn put_record(&self, key: &str, record: &McpTaskRecord) -> crate::cache::Result<()> {
        self.cache
            .put_entity(TASK_ENTITY_TYPE, key, serde_json::to_value(record)?)
            .await
    }
}

fn task_key(instance_id: InstanceId, task_id: &str) -> String {
    format!("{instance_id}::{task_id}")
}

impl McpTaskStatus {
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;

    use super::*;
    use crate::cache::{memory_cache_store, CacheLayerManager};
    use crate::identity::{ScopeRef, ServiceInstanceKey};

    fn instance(service_name: &str) -> InstanceId {
        ServiceInstanceKey::new(service_name, ScopeRef::Store).instance_id()
    }

    fn task(task_id: &str, status: McpTaskStatus, ttl_ms: Option<u64>) -> McpTask {
        let timestamp = Utc::now().to_rfc3339();
        McpTask {
            task_id: task_id.to_string(),
            status,
            status_message: None,
            created_at: timestamp.clone(),
            last_updated_at: timestamp,
            ttl_ms,
            poll_interval_ms: Some(25),
        }
    }

    #[tokio::test]
    async fn persists_task_records_and_rehydrates_them() {
        let backend = memory_cache_store();
        let first = TaskStateStore::new(Arc::new(CacheLayerManager::new(
            backend.clone(),
            "task-state",
        )));
        let instance_id = instance("alpha");
        let observed = first
            .observe(
                instance_id,
                task("task-1", McpTaskStatus::Working, Some(5_000)),
                Some("long_tool"),
            )
            .await
            .unwrap();

        let second = TaskStateStore::new(Arc::new(CacheLayerManager::new(backend, "task-state")));
        let records = second.list(instance_id).await.unwrap();
        assert_eq!(records, vec![observed]);
        assert_eq!(records[0].tool_name.as_deref(), Some("long_tool"));
        assert_eq!(records[0].task.poll_interval_ms, Some(25));
    }

    #[tokio::test]
    async fn task_ids_are_isolated_by_instance() {
        let state = TaskStateStore::new(Arc::new(CacheLayerManager::new(
            memory_cache_store(),
            "task-state",
        )));
        let first = state
            .observe(
                instance("alpha"),
                task("same-id", McpTaskStatus::Working, None),
                Some("alpha_tool"),
            )
            .await
            .unwrap();
        let second = state
            .observe(
                instance("beta"),
                task("same-id", McpTaskStatus::Completed, None),
                Some("beta_tool"),
            )
            .await
            .unwrap();

        assert_eq!(state.list(instance("alpha")).await.unwrap(), vec![first]);
        assert_eq!(state.list(instance("beta")).await.unwrap(), vec![second]);
    }

    #[tokio::test]
    async fn disconnect_marks_non_terminal_tasks_without_touching_terminal_tasks() {
        let state = TaskStateStore::new(Arc::new(CacheLayerManager::new(
            memory_cache_store(),
            "task-state",
        )));
        let instance_id = instance("alpha");
        state
            .observe(
                instance_id,
                task("working", McpTaskStatus::Working, None),
                Some("work"),
            )
            .await
            .unwrap();
        state
            .observe(
                instance_id,
                task("completed", McpTaskStatus::Completed, None),
                Some("done"),
            )
            .await
            .unwrap();

        state
            .mark_disconnected(instance_id, "transport closed")
            .await
            .unwrap();

        let records = state.list(instance_id).await.unwrap();
        let statuses = records
            .into_iter()
            .map(|record| (record.task_id, (record.task.status, record.last_error)))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(statuses["working"].0, McpTaskStatus::Disconnected);
        assert_eq!(statuses["working"].1.as_deref(), Some("transport closed"));
        assert_eq!(statuses["completed"].0, McpTaskStatus::Completed);
        assert_eq!(statuses["completed"].1, None);
    }

    #[tokio::test]
    async fn result_error_does_not_delete_task_record() {
        let state = TaskStateStore::new(Arc::new(CacheLayerManager::new(
            memory_cache_store(),
            "task-state",
        )));
        let instance_id = instance("alpha");
        state
            .observe(
                instance_id,
                task("task-1", McpTaskStatus::Completed, None),
                Some("long_tool"),
            )
            .await
            .unwrap();
        state
            .record_error(instance_id, "task-1", "result unavailable")
            .await
            .unwrap();

        let record = state.get(instance_id, "task-1").await.unwrap().unwrap();
        assert_eq!(record.task.status, McpTaskStatus::Completed);
        assert_eq!(record.last_error.as_deref(), Some("result unavailable"));
    }
}
