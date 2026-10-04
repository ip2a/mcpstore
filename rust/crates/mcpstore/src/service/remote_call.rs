//! 跨面板工具调用 RPC（方案 B）：面板之间没有连接，一切经共享库。
//!
//! 数据面板：`call_tool_remote` 先订响应流、再写请求、堵在 select 上等
//! 控制面板写回。控制面板：订阅 `tool_call_requests`，醒来原子认领
//! （claim = get_with_revision + compare_and_delete，恰好一个赢家），
//! 本地执行后把响应写进 `tool_call_responses`。
//!
//! 不支持流式响应：等结果全部完成才写响应。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use openkeyv::{AsyncChangeFeed, ChangeFeedRequest, ChangeFilter, ChangeOperation, ChangeStart};
use serde_json::Value;

use crate::cache::models::{ToolCallRequestEntity, ToolCallResponseEntity, ToolCallResponseError};
use crate::cache::CacheLayerManager;
use crate::store::prelude::*;
use crate::store::{MCPStore, PanelRole};

const REQUESTS: &str = "tool_call_requests";
const RESPONSES: &str = "tool_call_responses";
/// 数据面板等响应的上限；控制面板对超期请求认领后直接丢弃。
const CALL_DEADLINE: Duration = Duration::from_secs(30);
/// 响应 TTL：数据面板正常读走即删；它挂了由控制面板兜底清理。
const RESPONSE_TTL: Duration = Duration::from_secs(300);

impl MCPStore {
    /// 数据面板：请求写进共享库，等控制面板写回响应。
    /// ponytail: 每次调用新开一条订阅，量大时换成长连订阅 + 等待者表。
    pub(crate) async fn call_tool_remote(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: Value,
    ) -> Result<crate::transport::ToolCallResult> {
        let PanelRole::DataPanel { panel_id } = &self.kernel.runtime.panel_role else {
            return Err(Error::new(
                FailureCode::Internal,
                "remote tool call is data-panel only".to_string(),
            ));
        };
        let request_id = uuid::Uuid::new_v4().to_string();
        let response_key = format!("{panel_id}:{request_id}");

        // 先订后写：Latest 订阅看不到订阅前的写入，顺序反了就丢响应。
        let backend = self.ensure_event_backend().await?;
        let collection =
            CacheLayerManager::entity_collection_with_namespace(&self.namespace(), RESPONSES);
        let mut subscription = backend
            .subscribe(ChangeFeedRequest {
                start: ChangeStart::Latest,
                filter: ChangeFilter {
                    collections: vec![collection],
                    operations: vec![ChangeOperation::Put],
                },
            })
            .await
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;

        let request = serde_json::to_value(ToolCallRequestEntity {
            request_id: request_id.clone(),
            panel_id: panel_id.clone(),
            instance_id,
            tool_name: tool_name.to_string(),
            arguments: args,
            deadline_unix_ms: (chrono::Utc::now() + CALL_DEADLINE).timestamp_millis(),
        })
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        self.cache()
            .put_entity(REQUESTS, &request_id, request)
            .await?;

        loop {
            tokio::select! {
                change = subscription.recv() => {
                    let change = change
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?
                        .ok_or_else(|| {
                            Error::new(FailureCode::Internal, "tool call feed closed".to_string())
                        })?;
                    if change.key != response_key {
                        continue;
                    }
                    let value = self
                        .cache()
                        .get_entity(RESPONSES, &response_key)
                        .await?
                        .ok_or_else(|| {
                            Error::new(
                                FailureCode::Internal,
                                "tool call response vanished".to_string(),
                            )
                        })?;
                    self.cache().delete_entity(RESPONSES, &response_key).await.ok();
                    let response: ToolCallResponseEntity = serde_json::from_value(value)
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
                    return match (response.result, response.error) {
                        (Some(result), _) => Ok(result),
                        (None, Some(error)) => Err(remote_error(&error)),
                        (None, None) => Err(Error::new(
                            FailureCode::Internal,
                            "tool call response carries neither result nor error".to_string(),
                        )),
                    };
                }
                _ = tokio::time::sleep(CALL_DEADLINE) => {
                    // 超时删请求；控制面板若已认领，响应由 TTL 兜底清理。
                    self.cache().delete_entity(REQUESTS, &request_id).await.ok();
                    return Err(Error::new(
                        FailureCode::CallTimedOut,
                        format!("remote tool call '{tool_name}' timed out"),
                    ));
                }
            }
        }
    }

    /// 控制面板：订阅工具调用请求并代理执行。setup（或 load_from_config）后调用。
    pub(crate) fn spawn_tool_call_request_feed(self: &Arc<Self>) {
        if !matches!(self.kernel.runtime.panel_role, PanelRole::ControlPanel) {
            return;
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        if self
            .kernel
            .runtime
            .tool_call_feed_started
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let store = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match store.consume_tool_call_requests().await {
                    Ok(()) => {}
                    Err(error) if error.message().contains("does not provide ChangeFeed") => {
                        tracing::warn!("[STORE] tool call feed: {error}");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!("[STORE] tool call feed resubscribe: {error}");
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            store
                .kernel
                .runtime
                .tool_call_feed_started
                .store(false, Ordering::Release);
        });
    }

    pub(crate) fn spawn_tool_call_request_feed_from_ref(&self) {
        let Some(store) = self
            .kernel
            .runtime
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
        else {
            return;
        };
        store.spawn_tool_call_request_feed();
    }

    async fn consume_tool_call_requests(self: &Arc<Self>) -> Result<()> {
        let backend = self.ensure_event_backend().await?;
        let collection =
            CacheLayerManager::entity_collection_with_namespace(&self.namespace(), REQUESTS);
        let mut subscription = backend
            .subscribe(ChangeFeedRequest {
                start: ChangeStart::Latest,
                filter: ChangeFilter {
                    collections: vec![collection],
                    operations: vec![ChangeOperation::Put],
                },
            })
            .await
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        // 订上再扫一遍已有请求：控制面板启动前数据面板写下的、还没过期的。
        self.drain_tool_call_requests().await;
        loop {
            tokio::select! {
                change = subscription.recv() => match change {
                    Ok(Some(change)) => {
                        let store = Arc::clone(self);
                        // 工具调用可能很慢，逐条 spawn，不阻塞消费循环。
                        // ponytail: 无并发上限，压力大时加信号量。
                        let request_id = change.key;
                        tokio::spawn(async move {
                            store.handle_tool_call_request(&request_id).await;
                        });
                    }
                    Ok(None) => return Ok(()),
                    Err(error) => {
                        return Err(Error::new(FailureCode::Internal, error.to_string()));
                    }
                },
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // 兜底重扫：换库自愈（drain 走 active store）+ 漏推送恢复。
                    // 超期请求认领后即弃，重扫不会复活它们。
                    self.drain_tool_call_requests().await;
                }
            }
        }
    }

    async fn drain_tool_call_requests(self: &Arc<Self>) {
        let Ok(requests) = self.cache().get_all_entities_async(REQUESTS).await else {
            return;
        };
        for request_id in requests.keys() {
            let store = Arc::clone(self);
            let request_id = request_id.clone();
            tokio::spawn(async move {
                store.handle_tool_call_request(&request_id).await;
            });
        }
    }

    async fn handle_tool_call_request(self: &Arc<Self>, request_id: &str) {
        let Some(value) = self
            .cache()
            .claim_entity(REQUESTS, request_id)
            .await
            .ok()
            .flatten()
        else {
            // 不存在或已被别人原子认领
            return;
        };
        let request: ToolCallRequestEntity = match serde_json::from_value(value) {
            Ok(request) => request,
            Err(error) => {
                tracing::warn!("[STORE] tool call request {request_id} unreadable: {error}");
                return;
            }
        };
        if chrono::Utc::now().timestamp_millis() > request.deadline_unix_ms {
            tracing::debug!("[STORE] tool call request {request_id} expired; dropped");
            return;
        }

        // 先建连再 call：override 解析按已同步的 tools 校验工具名（与本地路径顺序一致）。
        let started = std::time::Instant::now();
        let executed = async {
            self.ensure_instance_connected(request.instance_id).await?;
            self.call_tool(
                request.instance_id,
                &request.tool_name,
                request.arguments.clone(),
            )
            .await
        }
        .await;
        tracing::info!(
            "[STORE] remote tool call '{}' for panel '{}' took {}ms: {}",
            request.tool_name,
            request.panel_id,
            started.elapsed().as_millis(),
            if executed.is_ok() { "ok" } else { "failed" }
        );
        let response = match executed {
            Ok(result) => ToolCallResponseEntity {
                request_id: request.request_id.clone(),
                panel_id: request.panel_id.clone(),
                result: Some(result),
                error: None,
            },
            Err(error) => ToolCallResponseEntity {
                request_id: request.request_id.clone(),
                panel_id: request.panel_id.clone(),
                result: None,
                error: Some(ToolCallResponseError {
                    code: error.code(),
                    message: error.message().to_string(),
                }),
            },
        };

        let response_key = format!("{}:{}", request.panel_id, request.request_id);
        let payload = serde_json::to_value(&response).unwrap_or_default();
        if let Err(error) = self
            .cache()
            .put_entity(RESPONSES, &response_key, payload)
            .await
        {
            tracing::error!("[STORE] tool call response {response_key} unwritable: {error}");
            return;
        }

        // TTL 兜底：数据面板挂了没人读走响应，5 分钟后清掉。
        let store = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(RESPONSE_TTL).await;
            store
                .cache()
                .delete_entity(RESPONSES, &response_key)
                .await
                .ok();
        });
    }
}

fn remote_error(error: &ToolCallResponseError) -> Error {
    Error::new(error.code, error.message.clone())
}
