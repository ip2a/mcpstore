use futures_util::StreamExt;

use super::catalog::{catalog_name_counts, service_namespace};
use super::tools::{read_required_instance_id, read_required_object, read_required_string};
use super::*;
use mcpstore::telemetry::FrameTelemetry;

pub(super) async fn connect_target_instances(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
) -> Result<(), BoxErr> {
    let instances = store.list_scope_instances(scope).await?;
    let instance_ids = if let Some(instance_id) = target_instance_id {
        if !instances
            .iter()
            .any(|instance| instance.instance_id == instance_id)
        {
            return Err(
                format!("Instance {instance_id} is not declared in scope {scope:?}").into(),
            );
        }
        vec![instance_id]
    } else {
        instances
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect()
    };

    for instance_id in instance_ids {
        store.connect_service(instance_id).await?;
    }
    Ok(())
}

pub(super) async fn build_tool_bindings(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
    session_key: Option<&str>,
) -> Result<HashMap<String, ToolBinding>, BoxErr> {
    let tool_payloads = if let Some(session_key) = session_key {
        serde_json::to_value(store.list_tools_in_session(session_key).await?)
            .and_then(serde_json::from_value::<Vec<Value>>)
            .map_err(|error| format!("session tool metadata serialization failed: {error}"))?
    } else if let Some(instance_id) = target_instance_id {
        store
            .list_tools_for_instance_with_filter(
                instance_id,
                mcpstore::ToolVisibilityFilter::Available,
            )
            .await?
    } else {
        store.list_tools_scoped(scope).await?
    };

    let names = catalog_name_counts(&tool_payloads, "name")?;
    let namespace_tools = target_instance_id.is_none() && session_key.is_none();

    let mut bindings = HashMap::with_capacity(tool_payloads.len());
    for payload in tool_payloads {
        let original_name = read_required_string(&payload, "name")?;
        let canonical_tool_name = read_required_string(&payload, "tool_name")?;
        let instance_id = read_required_instance_id(&payload, "instance_id")?;
        let service_name = read_required_string(&payload, "service_name")?;
        let exposed_name =
            if namespace_tools || names.get(&original_name).copied().unwrap_or_default() > 1 {
                format!("{}__{}", service_namespace(&service_name), original_name)
            } else {
                original_name
            };
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let schema = read_required_object(&payload, "input_schema")?;

        let tool = Tool::new_with_raw(
            exposed_name.clone(),
            description.clone().map(Into::into),
            Arc::new(schema),
        );
        if bindings
            .insert(
                exposed_name.clone(),
                ToolBinding {
                    tool,
                    instance_id,
                    tool_name: canonical_tool_name,
                    service_name,
                },
            )
            .is_some()
        {
            return Err(format!(
                "tool name projection conflict, cannot build the gateway MCP surface: scope={scope:?} tool={exposed_name}"
            )
            .into());
        }
    }

    Ok(bindings)
}

/// Build the gateway's streamable-http axum router (no bind; the caller owns the listener).
pub(crate) fn streamable_http_router(server: GatewayServer, path: &str) -> axum::Router {
    let frame_sink = gateway_frame_sink(&server);
    let caller = server.caller.clone();
    let service: StreamableHttpService<GatewayServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(server.clone()),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)
                .with_stateless_protocol_metadata_required(true),
        );
    axum::Router::new()
        .nest_service(&normalize_http_path(path), service)
        .layer(axum::middleware::from_fn(move |request, next| {
            record_gateway_frames(frame_sink.clone(), caller.clone(), request, next)
        }))
}

/// The store-attached sink on in-daemon routers; a standalone gateway (no
/// daemon opened telemetry) opens the shared telemetry file itself on first
/// build. Failure to open never blocks the gateway.
fn gateway_frame_sink(server: &GatewayServer) -> Option<std::sync::Arc<dyn FrameTelemetry>> {
    if let Some(sink) = server.store.frame_telemetry() {
        return Some(sink);
    }
    match crate::telemetry::TelemetryLog::open(crate::telemetry::default_db_path()) {
        Ok(log) => {
            let sink: std::sync::Arc<dyn FrameTelemetry> = log;
            server.store.set_frame_telemetry(Some(sink.clone()));
            Some(sink)
        }
        Err(error) => {
            tracing::warn!(%error, "gateway frame telemetry unavailable");
            None
        }
    }
}

/// Cap on buffered gateway bodies. GET streams are never buffered at all
/// (see [`record_gateway_frames`]); SSE POST responses stream through with a
/// line tee instead of buffering.
const MAX_FRAME_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Records every JSON-RPC envelope crossing the gateway's HTTP boundary:
/// request body in = ↑, response body out = ↓, grouped by the client's
/// `Mcp-Session-Id` (stateless requests without one share a mount-level
/// session). The gateway serves each POST statelessly — rmcp builds a
/// per-request `OneshotTransport` internally — so there is no persistent
/// `Transport<RoleServer>` to tee; the HTTP body IS the frame.
///
/// Only POST crosses a recordable boundary: GET is the SSE listen channel
/// (unbounded stream), DELETE carries no JSON-RPC body.
async fn record_gateway_frames(
    frame_sink: Option<std::sync::Arc<dyn FrameTelemetry>>,
    caller: Option<String>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let Some(sink) = frame_sink else {
        return next.run(request).await;
    };
    if request.method() != axum::http::Method::POST {
        return next.run(request).await;
    }
    let session_id = request
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| format!("gw:{value}"))
        .unwrap_or_else(|| {
            format!(
                "gw:stateless:{}",
                caller.clone().unwrap_or_else(|| "direct".to_string())
            )
        });
    let (parts, body) = request.into_parts();
    let request_bytes = axum::body::to_bytes(body, MAX_FRAME_BODY_BYTES)
        .await
        .unwrap_or_default();
    emit_gateway_frames(&sink, &session_id, "up", &caller, &request_bytes);
    let response = next
        .run(axum::extract::Request::from_parts(
            parts,
            axum::body::Body::from(request_bytes),
        ))
        .await;
    let (parts, body) = response.into_parts();
    let is_sse = parts
        .headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    if is_sse {
        // Stream through untouched; tee complete `data:` lines as they flow so
        // long-running tools keep their incremental progress pushes.
        let stream = body.into_data_stream().map(move |chunk| {
            if let Ok(bytes) = &chunk {
                emit_gateway_frames(&sink, &session_id, "down", &caller, bytes);
            }
            chunk
        });
        return axum::response::Response::from_parts(parts, axum::body::Body::from_stream(stream));
    }
    let response_bytes = axum::body::to_bytes(body, MAX_FRAME_BODY_BYTES)
        .await
        .unwrap_or_default();
    emit_gateway_frames(&sink, &session_id, "down", &caller, &response_bytes);
    axum::response::Response::from_parts(parts, axum::body::Body::from(response_bytes))
}

/// Parse every JSON-RPC envelope inside `bytes` (a POST body is one envelope;
/// an SSE chunk contributes its complete `data:` lines) and record a frame per
/// envelope. Non-JSON bytes are skipped — only valid envelopes are frames.
fn emit_gateway_frames(
    sink: &std::sync::Arc<dyn FrameTelemetry>,
    session_id: &str,
    direction: &str,
    caller: &Option<String>,
    bytes: &[u8],
) {
    let text = String::from_utf8_lossy(bytes);
    let mut envelopes = Vec::new();
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(data.trim()) {
                envelopes.push(envelope);
            }
        } else if line.trim_start().starts_with('{') {
            if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                envelopes.push(envelope);
            }
        }
    }
    for envelope in envelopes {
        let mut record = mcpstore::telemetry::FrameRecord::new(session_id, "gateway", direction);
        record.caller = caller.clone();
        record.transport = Some("streamable-http".to_string());
        // The client identity rides on the initialize handshake.
        if envelope.get("method").and_then(|value| value.as_str()) == Some("initialize") {
            record.peer = envelope
                .pointer("/params/clientInfo/name")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        record.from_envelope(&envelope);
        record.payload = envelope.to_string();
        record.size_bytes = record.payload.len() as i64;
        sink.record_frame(record);
    }
}

pub(super) fn normalize_http_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/mcp".to_string();
    }
    let normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    normalized.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use std::time::Duration;

    async fn json_reply() -> axum::Json<serde_json::Value> {
        axum::Json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{"tools":[]}}))
    }

    async fn sse_reply() -> impl axum::response::IntoResponse {
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"done\":true}}\n\n",
        )
    }

    fn frame_router(sink: std::sync::Arc<crate::telemetry::TelemetryLog>) -> axum::Router {
        let dyn_sink: std::sync::Arc<dyn FrameTelemetry> = sink;
        axum::Router::new()
            .route("/mcp", axum::routing::post(json_reply))
            .route(
                "/mcp/sse",
                axum::routing::post(sse_reply).get(|| async { "stream" }),
            )
            .layer(axum::middleware::from_fn(move |request, next| {
                record_gateway_frames(
                    Some(std::sync::Arc::clone(&dyn_sink)),
                    Some("claude".to_string()),
                    request,
                    next,
                )
            }))
    }

    #[tokio::test]
    async fn records_json_post_pair_with_initialize_identity() {
        let dir = std::env::temp_dir().join(format!("mcpstore-frames-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gateway-frames.db");
        let _ = std::fs::remove_file(&path);
        let log = crate::telemetry::TelemetryLog::open(&path).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = frame_router(std::sync::Arc::clone(&log));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::new();

        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2026-07-28",
                "clientInfo": { "name": "claude-code", "version": "1.0" },
                "capabilities": {}
            }
        });
        let response = client
            .post(format!("http://{addr}/mcp"))
            .json(&initialize)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        std::thread::sleep(Duration::from_millis(300));

        let frames = log.frame_history("gw:stateless:claude", 50).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].face, "gateway");
        assert_eq!(frames[0].direction, "up");
        assert_eq!(frames[0].method.as_deref(), Some("initialize"));
        assert_eq!(frames[0].kind, "request");
        assert_eq!(frames[0].caller.as_deref(), Some("claude"));
        assert_eq!(frames[0].peer.as_deref(), Some("claude-code"));
        assert_eq!(frames[0].jsonrpc_id.as_deref(), Some("1"));

        assert_eq!(frames[1].direction, "down");
        assert_eq!(frames[1].kind, "response");
        assert_eq!(frames[1].method, None);
        assert_eq!(frames[1].transport.as_deref(), Some("streamable-http"));
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn records_sse_data_lines_and_skips_get() {
        let dir = std::env::temp_dir().join(format!("mcpstore-frames-sse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gateway-frames.db");
        let _ = std::fs::remove_file(&path);
        let log = crate::telemetry::TelemetryLog::open(&path).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = frame_router(std::sync::Arc::clone(&log));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::new();

        // SSE POST: the data-line frame is recorded, body still streams through.
        let response = client
            .post(format!("http://{addr}/mcp/sse"))
            .send()
            .await
            .unwrap();
        let body = response.text().await.unwrap();
        assert!(body.contains("\"done\":true"));
        // GET never records.
        client
            .get(format!("http://{addr}/mcp/sse"))
            .send()
            .await
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));

        // No request body was sent; only the response data line became a frame.
        let frames = log.frame_history("gw:stateless:claude", 50).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].direction, "down");
        assert_eq!(frames[0].jsonrpc_id.as_deref(), Some("2"));
        assert_eq!(frames[0].kind, "response");
        std::fs::remove_file(&path).ok();
    }
}
