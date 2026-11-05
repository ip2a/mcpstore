use futures_util::StreamExt;

use super::catalog::{
    apply_service_overrides, catalog_name_counts, exposed_tool_name, service_override_map,
};
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
    let overrides_by_instance = service_override_map(store).await?;
    let tool_payloads = apply_service_overrides(tool_payloads, &overrides_by_instance)?;

    let names = catalog_name_counts(&tool_payloads, "name")?;
    let namespace_tools = target_instance_id.is_none() && session_key.is_none();

    let mut bindings = HashMap::with_capacity(tool_payloads.len());
    let mut taken: HashMap<String, usize> = HashMap::new();
    for payload in tool_payloads {
        let original_name = read_required_string(&payload, "name")?;
        let canonical_tool_name = read_required_string(&payload, "tool_name")?;
        let instance_id = read_required_instance_id(&payload, "instance_id")?;
        let service_name = read_required_string(&payload, "service_name")?;

        let exposed_name = if namespace_tools
            || names.get(&original_name).copied().unwrap_or_default() > 1
        {
            // ponytail: magpie-style tool naming — <server>__<tool>, unsafe chars → '-',
            // 64-byte rune-safe truncation, collision suffix (_2, _3, …) budget within 64.
            exposed_tool_name(&service_name, &original_name, &mut taken)
        } else {
            original_name
        };
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let schema = read_required_object(&payload, "input_schema")?;

        let mut tool = Tool::new_with_raw(
            exposed_name.clone(),
            description.clone().map(Into::into),
            Arc::new(schema),
        );
        tool.title = payload.get("title").and_then(Value::as_str).map(str::to_string);
        tool.icons = payload
            .get("icons")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
        tool.annotations = payload
            .get("annotations")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
        tool.meta = payload
            .get("_meta")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
        // The call log joins with the services page, which shows real names.
        let record_service_name = payload
            .get("real_service_name")
            .and_then(Value::as_str)
            .unwrap_or(&service_name)
            .to_string();
        bindings.insert(
            exposed_name.clone(),
            ToolBinding {
                tool,
                instance_id,
                tool_name: canonical_tool_name,
                service_name: record_service_name,
            },
        );
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
            // Inclusive gateway: every request is served statelessly, but the
            // per-request SEP-2575/SEP-2243 metadata stays optional — bare
            // legacy callers (curl, old SDKs) are served as 2025-03-26, and
            // clients that do send the metadata keep working. JSON responses
            // for simple request/response, SSE fallback when a notification
            // precedes the response.
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)
                .with_json_response(true),
        );
    axum::Router::new()
        .nest_service(&normalize_http_path(path), service)
        .layer(axum::middleware::from_fn(move |request, next| {
            record_gateway_frames(frame_sink.clone(), caller.clone(), request, next)
        }))
        .layer(axum::middleware::from_fn(relax_stateless_contract))
}

/// Body size cap for the inclusive-gateway shim; requests larger than this
/// pass through untouched (rmcp enforces its own body limit).
const RELAX_STATELESS_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// Inclusive-gateway shim: absorb protocol gaps for callers that don't
/// implement the stateless request contract (curl, quick scripts, older
/// SDKs). Only ever ADDS what is absent — a caller's explicit header or
/// `_meta` is never rewritten, and partial `_meta` passes through so rmcp's
/// validation still reports exactly what a half-implemented client must add.
///
/// - Widens an incomplete `Accept` header to the streamable contract
///   (`application/json, text/event-stream`); rmcp 406s anything narrower,
///   including curl's default `*/*`.
/// - Requests with no `_meta` at all get the two stateless-required keys
///   injected (`io.modelcontextprotocol/protocolVersion`, and an empty
///   `io.modelcontextprotocol/clientCapabilities`). The version comes from
///   the `MCP-Protocol-Version` header when present (header and `_meta` must
///   agree); with no header both default to `2025-03-26`, matching rmcp's
///   bare-request peer assumption.
async fn relax_stateless_contract(
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let accept = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !accept.contains("application/json") || !accept.contains("text/event-stream") {
        request.headers_mut().insert(
            axum::http::header::ACCEPT,
            axum::http::HeaderValue::from_static("application/json, text/event-stream"),
        );
    }

    if request.method() == axum::http::Method::POST {
        let (mut parts, body) = request.into_parts();
        let header_version = parts
            .headers
            .get("MCP-Protocol-Version")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = match axum::body::to_bytes(body, RELAX_STATELESS_BODY_LIMIT).await {
            Ok(bytes) => bytes,
            // Same rejection rmcp would produce for an oversized body.
            Err(_) => {
                use axum::response::IntoResponse;
                return axum::http::StatusCode::PAYLOAD_TOO_LARGE.into_response();
            }
        };
        let bytes = if bytes.is_empty() {
            bytes
        } else {
            match inject_stateless_meta(&bytes, header_version) {
                Some((json, echo_version)) => {
                    // rmcp rejects `_meta.protocolVersion` without a matching
                    // `MCP-Protocol-Version` header; echo one only when the
                    // caller sent none (an existing header is never rewritten).
                    if let Some(version) = echo_version {
                        if let Ok(value) = axum::http::HeaderValue::from_str(&version) {
                            parts.headers.insert("MCP-Protocol-Version", value);
                        }
                    }
                    json
                }
                None => bytes,
            }
        };
        request = axum::extract::Request::from_parts(parts, axum::body::Body::from(bytes));
    }

    next.run(request).await
}

/// Fill the stateless-required request metadata a caller omitted: the
/// `io.modelcontextprotocol/protocolVersion` key and the
/// `io.modelcontextprotocol/clientCapabilities` key. The version comes from
/// the caller's `MCP-Protocol-Version` header, else from their `_meta`, else
/// defaults to `2025-03-26` (rmcp's bare-request peer assumption). Returns
/// the rewritten body plus the version to echo into an absent
/// `MCP-Protocol-Version` header, or `None` when the body is not a JSON-RPC
/// request (`initialize`, notifications) or already carries everything.
fn inject_stateless_meta(
    body: &[u8],
    header_version: Option<String>,
) -> Option<(axum::body::Bytes, Option<String>)> {
    const META_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
    const META_CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";

    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let object = value.as_object_mut()?;
    if object.get("method").and_then(serde_json::Value::as_str) == Some("initialize") {
        return None;
    }
    if !object.contains_key("id") {
        return None; // notifications carry no request meta
    }
    let params = object
        .entry("params")
        .or_insert_with(|| serde_json::Value::Object(Default::default()));
    let params = params.as_object_mut()?;
    // Per-key fill: add only what the caller omitted. A caller that names a
    // version is treated as speaking the stateless contract — their values
    // are never rewritten, so an unsupported version still fails loudly in
    // dispatch with the supported list attached.
    let meta = match params.get_mut("_meta") {
        Some(serde_json::Value::Object(meta)) => meta,
        Some(_) => return None, // non-object _meta: caller's payload, untouched
        None => {
            params.insert("_meta".to_string(), serde_json::Value::Object(Default::default()));
            params
                .get_mut("_meta")
                .and_then(serde_json::Value::as_object_mut)
                .expect("just inserted")
        }
    };
    let meta_version = meta
        .get(META_VERSION_KEY)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let effective_version = match &meta_version {
        Some(version) => version.clone(),
        None => header_version.unwrap_or_else(|| "2025-03-26".to_string()),
    };
    let mut added = false;
    if meta_version.is_none() {
        meta.insert(META_VERSION_KEY.to_string(), serde_json::Value::String(effective_version.clone()));
        added = true;
    }
    if !meta.contains_key(META_CAPABILITIES_KEY) {
        meta.insert(META_CAPABILITIES_KEY.to_string(), serde_json::json!({}));
        added = true;
    }
    if !added {
        return None;
    }
    let bytes = serde_json::to_vec(&value).ok()?;
    Some((axum::body::Bytes::from(bytes), Some(effective_version)))
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
