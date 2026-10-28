use axum::{
    body::{Body, HttpBody},
    extract::{Path, State},
    http::{header, HeaderMap, HeaderName, Method, Request, StatusCode, Uri},
    response::Response,
};
use futures_util::TryStreamExt;
use mcpstore::config::HostEntry;
use reqwest::Url;
use serde::Serialize;
use serde_json::json;

use super::{
    envelope::{success, ApiError, ApiResult},
    ApiState,
};

pub(super) const HOST_HEADER: &str = "x-mcpstore-host";

#[derive(Debug, Serialize)]
pub(super) struct HostStatus {
    name: String,
    online: bool,
    url: Option<String>,
    node_id: Option<String>,
    version: Option<String>,
    store: Option<String>,
    latency_ms: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteMetaEnvelope {
    data: Option<RemoteMeta>,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteMeta {
    version: Option<String>,
    node: Option<RemoteNode>,
    features: Option<RemoteFeatures>,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteFeatures {
    app_host_routing_v1: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteNode {
    id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteHealth {
    store: Option<String>,
}

pub(super) async fn host_status(
    State(state): State<std::sync::Arc<ApiState>>,
    Path(name): Path<String>,
) -> ApiResult {
    let config = state
        .store()
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let entry = config.hosts.entries.get(&name).ok_or_else(|| {
        ApiError::not_found(
            mcpstore::error::FailureCode::InvalidInput,
            format!("host {name} is not configured"),
            Some("host"),
            None,
        )
    })?;
    let status = probe_host(&name, entry, &state.http_client).await?;
    Ok(success(
        "Host status fetched",
        serde_json::to_value(status)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn reject_selected_host(
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    if request.headers().contains_key(HOST_HEADER) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "host selector is not valid for a panel-local endpoint",
        );
    }
    next.run(request).await
}

pub(super) async fn route_selected_host(
    State(state): State<std::sync::Arc<ApiState>>,
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let Some(raw_name) = parts.headers.get(HOST_HEADER) else {
        return next.run(Request::from_parts(parts, body)).await;
    };
    let name = match raw_name.to_str() {
        Ok(value) if !value.is_empty() => value.to_owned(),
        _ => return error_response(StatusCode::BAD_REQUEST, "invalid host selector"),
    };
    let config = match state.store().config_manager().load_app_config_or_default() {
        Ok(config) => config,
        Err(error) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string());
        }
    };
    let Some(entry) = config.hosts.entries.get(&name) else {
        return error_response(StatusCode::BAD_REQUEST, "host is not configured");
    };
    let client = state.http_client.clone();
    match forward(&client, entry, Request::from_parts(parts, body)).await {
        Ok(response) => response,
        Err((status, message)) => error_response(status, &message),
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "success": false,
                "message": message,
                "data": null,
                "errors": [{
                    "code": "host_routing_error",
                    "message": message,
                    "field": null,
                    "details": null
                }],
                "meta": {
                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    "request_id": "host-routing",
                    "execution_time_ms": 0,
                    "api_version": "1.0.0"
                },
                "pagination": null
            })
            .to_string(),
        ))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn forward(
    client: &reqwest::Client,
    entry: &HostEntry,
    request: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_owned();
    let request_headers = parts.headers;
    let empty_body = body.size_hint().exact() == Some(0);
    let mut body_stream = (!empty_body).then(|| {
        body.into_data_stream()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()))
    });
    let can_retry = empty_body && is_retryable(&method);
    let mut last_error = None;
    for (index, base) in entry.urls.iter().enumerate() {
        let url = join_url(base, &path_and_query)?;
        let mut builder = client.request(method.clone(), url);
        if let Some(stream) = body_stream.take() {
            builder = builder.body(reqwest::Body::wrap_stream(stream));
        }
        let builder = copy_request_headers(&request_headers, builder);
        match builder.send().await {
            Ok(response) => return convert_response(response),
            Err(error) if can_retry && index + 1 < entry.urls.len() => {
                last_error = Some(error.to_string());
            }
            Err(error) => {
                return Err((
                    if error.is_timeout() {
                        StatusCode::GATEWAY_TIMEOUT
                    } else {
                        StatusCode::BAD_GATEWAY
                    },
                    format!("host request failed: {error}"),
                ));
            }
        }
    }
    Err((
        StatusCode::BAD_GATEWAY,
        last_error.unwrap_or_else(|| "host has no reachable URL".to_owned()),
    ))
}

fn join_url(base: &str, path_and_query: &str) -> Result<Url, (StatusCode, String)> {
    let mut base =
        Url::parse(base).map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    if !matches!(base.scheme(), "http" | "https")
        || !base.username().is_empty()
        || base.password().is_some()
        || base.fragment().is_some()
        || base.query().is_some()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "host URL must be a bare http(s) origin".to_owned(),
        ));
    }
    let uri = Uri::try_from(path_and_query)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let base_path = base.path().trim_end_matches('/');
    let request_path = uri.path().trim_start_matches('/');
    let path = if base_path.is_empty() {
        format!("/{request_path}")
    } else if request_path.is_empty() {
        format!("{base_path}/")
    } else {
        format!("{base_path}/{request_path}")
    };
    base.set_path(&path);
    base.set_query(uri.query());
    Ok(base)
}

fn copy_request_headers(
    headers: &HeaderMap,
    mut builder: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    for (name, value) in headers {
        if is_hop_by_hop(name)
            || matches!(
                name.as_str(),
                "authorization" | "cookie" | "origin" | "referer" | HOST_HEADER
            )
        {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "content-length"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn is_retryable(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn convert_response(response: reqwest::Response) -> Result<Response, (StatusCode, String)> {
    let status = response.status();
    let headers = response.headers().clone();
    let body = Body::from_stream(
        response
            .bytes_stream()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string())),
    );
    let mut builder = Response::builder().status(status);
    for (header_name, value) in &headers {
        if !is_hop_by_hop(header_name) {
            builder = builder.header(header_name, value);
        }
    }
    builder
        .body(body)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn probe_host(
    name: &str,
    entry: &HostEntry,
    client: &reqwest::Client,
) -> Result<HostStatus, ApiError> {
    let started = std::time::Instant::now();
    let mut last_error = None;
    for base in &entry.urls {
        let health_url =
            join_url(base, "/health").map_err(|(_, message)| ApiError::invalid_request(message))?;
        let meta_url = join_url(base, "/v1/daemon/meta")
            .map_err(|(_, message)| ApiError::invalid_request(message))?;
        let health = client.get(health_url).send().await;
        let meta = client.get(meta_url).send().await;
        match (health, meta) {
            (Ok(health), Ok(meta))
                if health.status().is_success() && meta.status().is_success() =>
            {
                let health = health.json::<RemoteHealth>().await.ok();
                let meta = meta
                    .json::<RemoteMetaEnvelope>()
                    .await
                    .ok()
                    .and_then(|body| body.data);
                let node_id = meta
                    .as_ref()
                    .and_then(|value| value.node.as_ref())
                    .and_then(|value| value.id.clone());
                let capable = meta
                    .as_ref()
                    .and_then(|value| value.features.as_ref())
                    .and_then(|value| value.app_host_routing_v1)
                    .unwrap_or(false);
                if !capable {
                    last_error = Some(format!("host {base} lacks app_host_routing_v1"));
                    continue;
                }
                return Ok(HostStatus {
                    name: name.to_owned(),
                    online: true,
                    url: Some(base.clone()),
                    node_id,
                    version: meta.as_ref().and_then(|value| value.version.clone()),
                    store: health.and_then(|value| value.store),
                    latency_ms: Some(started.elapsed().as_millis() as u64),
                    error: None,
                });
            }
            _ => last_error = Some(format!("host {base} did not pass health/meta")),
        }
    }
    Ok(HostStatus {
        name: name.to_owned(),
        online: false,
        url: None,
        node_id: None,
        version: None,
        store: None,
        latency_ms: None,
        error: last_error.or_else(|| Some("host has no reachable URL".to_owned())),
    })
}

#[cfg(test)]
mod tests {
    use super::{forward, join_url};
    use axum::{
        body::{to_bytes, Body},
        extract::State,
        http::{HeaderMap, Request, StatusCode, Uri},
        response::Response,
        routing::any,
        Router,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Capture {
        request: Arc<Mutex<Option<CapturedRequest>>>,
    }

    #[derive(Clone)]
    struct CapturedRequest {
        headers: HeaderMap,
        uri: Uri,
        body: Vec<u8>,
    }

    async fn capture_request(State(capture): State<Capture>, request: Request<Body>) -> Response {
        let (parts, body) = request.into_parts();
        let body = to_bytes(body, 1024 * 1024).await.unwrap().to_vec();
        *capture.request.lock().unwrap() = Some(CapturedRequest {
            headers: parts.headers,
            uri: parts.uri,
            body,
        });
        Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header("x-upstream", "preserved")
            .body(Body::from("upstream-body"))
            .unwrap()
    }

    async fn spawn_upstream(capture: Capture) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .fallback(any(capture_request))
            .with_state(capture);
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (url, handle)
    }

    #[test]
    fn host_url_preserves_base_path_and_rejects_unsafe_bases() {
        assert_eq!(
            join_url("http://127.0.0.1:1820/api/", "/services/list")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:1820/api/services/list"
        );
        assert!(join_url("file:///tmp", "/services/list").is_err());
        assert!(join_url("http://user@127.0.0.1:1820", "/services/list").is_err());
    }

    #[tokio::test]
    async fn forward_filters_sensitive_headers_and_preserves_response() {
        let capture = Capture::default();
        let (url, handle) = spawn_upstream(capture.clone()).await;
        let entry = mcpstore::config::HostEntry { urls: vec![url] };
        let request = Request::builder()
            .method("GET")
            .uri("/services/list?scope=store")
            .header("authorization", "Bearer secret")
            .header("cookie", "session=secret")
            .header("origin", "https://panel.example")
            .header("referer", "https://panel.example/")
            .header("connection", "keep-alive")
            .header("x-mcpstore-host", "remote")
            .header("x-forwarded-test", "kept")
            .body(Body::empty())
            .unwrap();

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let response = forward(&client, &entry, request).await.unwrap();
        assert_eq!(response.status(), StatusCode::MULTI_STATUS);
        assert_eq!(response.headers()["x-upstream"], "preserved");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            "upstream-body"
        );

        let captured = capture.request.lock().unwrap().clone().unwrap();
        assert_eq!(
            captured.uri.path_and_query().unwrap().as_str(),
            "/services/list?scope=store"
        );
        assert_eq!(captured.headers["x-forwarded-test"], "kept");
        for name in [
            "authorization",
            "cookie",
            "origin",
            "referer",
            "connection",
            "x-mcpstore-host",
        ] {
            assert!(!captured.headers.contains_key(name), "forwarded {name}");
        }
        handle.abort();
    }

    #[tokio::test]
    async fn forward_retries_get_but_never_retries_post() {
        let first_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_url = format!("http://{}", first_listener.local_addr().unwrap());
        drop(first_listener);

        let get_capture = Capture::default();
        let (get_url, get_handle) = spawn_upstream(get_capture.clone()).await;
        let get_entry = mcpstore::config::HostEntry {
            urls: vec![first_url.clone(), get_url],
        };
        let get_request = Request::builder()
            .method("GET")
            .uri("/retry")
            .body(Body::empty())
            .unwrap();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        assert_eq!(
            forward(&client, &get_entry, get_request)
                .await
                .unwrap()
                .status(),
            StatusCode::MULTI_STATUS
        );
        assert!(get_capture.request.lock().unwrap().is_some());
        get_handle.abort();

        let post_capture = Capture::default();
        let (post_url, post_handle) = spawn_upstream(post_capture.clone()).await;
        let post_entry = mcpstore::config::HostEntry {
            urls: vec![first_url, post_url],
        };
        let post_request = Request::builder()
            .method("POST")
            .uri("/no-retry")
            .body(Body::from("payload"))
            .unwrap();
        let error = forward(&client, &post_entry, post_request)
            .await
            .unwrap_err();
        assert!(matches!(
            error.0,
            StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT
        ));
        assert!(post_capture.request.lock().unwrap().is_none());
        post_handle.abort();
    }
}
