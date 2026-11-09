//! Gateway HTTP guard chain, ported from magpie's incident-hardened
//! `cors.go` / `serveMCP` (see local/调研文档 §3.1 + §7). Every guard here
//! exists because a real attack defeated the code before it:
//!
//! 1. peer loopback — the gateway hands out the user's downstream tool
//!    calls; only local callers may drive it (Bearer key lands with the
//!    LAN-exposure milestone, until then loopback-only).
//! 2. Host check — DNS rebinding: a web page's host resolving to 127.0.0.1
//!    looks "same-origin" to the browser; a GET carries no Origin so an
//!    Origin-only check loses. Host must be an exact loopback name.
//!    `*.localhost` fails too: WebKit resolves those via the system DNS
//!    (magpie #2256) and there is no key mechanism yet to hold them to.
//! 3. Origin check — `null` is refused (sandboxed iframe / file:// /
//!    webview, #2242); non-http(s) schemes pass (desktop apps, editor
//!    webviews and extensions are not CORS-bound and a web page cannot
//!    forge them); http(s) pages pass only at loopback names — a foreign
//!    page never read a reply before this guard, so refusing it breaks
//!    nothing that worked.
//! 4. POST-only — pure request/response Streamable HTTP; no GET SSE
//!    stream, no DELETE session to smuggle a cross-origin call through.
//! 5. Rate limit before execution — per-mount byte budget reserved before
//!    the body reaches the server, so an over-limit caller never runs a
//!    tool (magpie reserveFor, c431225a/6b6113aa).
//! 6. Body ceiling — enforced by rmcp itself (`max_request_body_bytes`,
//!    4 MiB default, streamed and Content-Length-independent), so no extra
//!    layer is needed here.
//!
//! Management routes (`/calls`, `/projection`) sit behind guards 1-2 only.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{self, HOST, ORIGIN};
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

// ponytail: fixed constants; move into [gateway] config when real usage
// shows a workload the defaults pinch.
const RATE_BURST_BYTES: f64 = 8.0 * 1024.0 * 1024.0;
const RATE_REFILL_PER_SEC: f64 = 2.0 * 1024.0 * 1024.0;
/// Cost charged when Content-Length is absent (chunked bodies).
const RATE_DEFAULT_COST: f64 = 4096.0;

/// Per-mount byte budget (magpie `reserveFor` semantics): reserve before
/// anything runs, refill continuously. A normal tool call costs a few KiB;
/// floods drain the bucket and get refused without executing.
pub(crate) struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn try_reserve(&self, key: &str, cost: f64) -> bool {
        let mut buckets = self.buckets.lock().expect("rate limiter poisoned");
        let now = Instant::now();
        let bucket = buckets
            .entry(key.to_string())
            .or_insert(Bucket {
                tokens: RATE_BURST_BYTES,
                last: now,
            });
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * RATE_REFILL_PER_SEC).min(RATE_BURST_BYTES);
        bucket.last = now;
        if bucket.tokens >= cost {
            bucket.tokens -= cost;
            true
        } else {
            false
        }
    }
}

/// Strip the port from a Host header value, handling `[::1]:9090` and bare
/// `::1` (invalid HTTP/1.1 but harmless to accept).
fn host_token(host_header: &str) -> &str {
    let host = host_header.trim();
    if let Some(rest) = host.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
    }
    match host.rsplit_once(':') {
        // a bare IPv6 has several colons; only strip when the left side is
        // colon-free (otherwise the split corrupted the address)
        Some((left, _)) if !left.contains(':') => left,
        _ => host,
    }
}

/// Host must be an exact loopback name — the rebinding guard. An attacker
/// domain resolving to 127.0.0.1 stays out; `*.localhost` stays out (WebKit
/// asks system DNS for those, magpie #2256, no key mechanism to exempt).
pub(crate) fn host_is_loopback(host_header: Option<&str>) -> bool {
    let Some(host_header) = host_header else {
        return true; // no Host: in-process caller (HTTP/2 :authority covers prod)
    };
    match host_token(host_header) {
        "localhost" | "127.0.0.1" | "::1" => true,
        other => other
            .parse::<IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false),
    }
}

/// Origin guard (magpie foreignPage + null + localhostSubdomain, reduced to
/// the no-key world): empty passes (every agent's requests), `null` is
/// refused, non-http(s) schemes pass, http(s) passes only at loopback names.
pub(crate) fn origin_refused(origin: Option<&str>) -> bool {
    let Some(origin) = origin.filter(|o| !o.is_empty()) else {
        return false;
    };
    if origin.eq_ignore_ascii_case("null") {
        return true;
    }
    let Ok(parsed) = url::Url::parse(origin) else {
        // a browser's origin always parses; an unreadable http-ish one is
        // no page of ours
        return origin.to_ascii_lowercase().starts_with("http");
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        // app://, file://, tauri://, vscode-webview://, chrome-extension://
        // come from desktop apps/webviews/extensions that don't enforce
        // CORS; no ordinary web page can send them
        return false;
    }
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    // the url crate renders IPv6 hosts bracketed
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host == "localhost" {
        return false;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip.is_loopback() {
            return false;
        }
    }
    // includes *.localhost: WebKit hands those to the system resolver, so
    // the page may not be on this computer
    true
}

fn text(status: StatusCode, message: &'static str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        message,
    )
        .into_response()
}

fn rate_limited() -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": {
            "code": -32000,
            "message": "rate limit exceeded for this mount",
            "data": {
                "code": "rate_limited",
                "retryable": true,
                "hint": "you are sending requests too fast for this mount; wait a moment and retry, and batch fewer calls per request"
            }
        }
    });
    (
        StatusCode::TOO_MANY_REQUESTS,
        [
            (header::RETRY_AFTER, "1"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The full guard chain as an axum middleware fn. `generic_path` is the CLI
/// launch path of the generic mount; wired mounts live at `/mcp/<id>`.
/// Mount with `axum::middleware::from_fn` at the call site.
pub(crate) async fn guard_chain(
    generic_path: String,
    limiter: Arc<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // 1. peer loopback
    let peer_loopback = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().is_loopback())
        .unwrap_or(true); // no connect info = in-process caller
    if !peer_loopback {
        return text(
            StatusCode::FORBIDDEN,
            "the mcp gateway answers loopback callers only",
        );
    }

    // 2. Host rebinding guard
    let host = request.headers().get(HOST).and_then(|v| v.to_str().ok());
    if !host_is_loopback(host) {
        return text(
            StatusCode::FORBIDDEN,
            "the mcp gateway answers loopback Host names only (dns rebinding guard)",
        );
    }

    let path = request.uri().path();
    let is_mcp = path == generic_path.as_str() || path.starts_with("/mcp/");
    if !is_mcp {
        // management surface: loopback + Host already enforced above
        return next.run(request).await;
    }

    // 3. Origin: agents, not web pages
    let origin = request.headers().get(ORIGIN).and_then(|v| v.to_str().ok());
    if origin_refused(origin) {
        return text(
            StatusCode::FORBIDDEN,
            "the mcp gateway answers agents, not web pages: open the page at localhost, or use an agent client",
        );
    }

    // 4. POST-only request/response surface
    if request.method() != Method::POST {
        let mut refusal = StatusCode::METHOD_NOT_ALLOWED.into_response();
        refusal
            .headers_mut()
            .insert(header::ALLOW, "POST".parse().expect("static header value"));
        return refusal;
    }

    // 5. Reserve against the mount's budget before anything runs
    let caller = path
        .strip_prefix("/mcp/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("generic")
        .to_string();
    let cost = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
        .unwrap_or(RATE_DEFAULT_COST);
    if !limiter.try_reserve(&caller, cost) {
        return rate_limited();
    }

    // 6. Body ceiling rides on rmcp's max_request_body_bytes (4 MiB).
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use tower::ServiceExt;

    const LOOPBACK: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        53190,
    );
    const LAN: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 5)),
        53190,
    );

    fn guarded_router() -> axum::Router {
        axum::Router::new()
            .route("/mcp/agent1", post(|| async { "mcp" }))
            .route("/mcp", post(|| async { "mcp" }))
            .route("/projection", get(|| async { "proj" }))
            .layer(axum::middleware::from_fn(move |request, next| {
                guard_chain("/mcp".to_string(), Arc::new(RateLimiter::new()), request, next)
            }))
    }

    fn request(method: Method, uri: &str) -> axum::extract::Request {
        let mut builder = Request::builder().method(method).uri(uri);
        builder = builder.header(HOST, "127.0.0.1:2893");
        let mut request = builder.body(Body::empty()).expect("static request");
        request.extensions_mut().insert(ConnectInfo(LOOPBACK));
        request
    }

    async fn status_of(request: axum::extract::Request) -> (StatusCode, Response) {
        let response = guarded_router().oneshot(request).await.expect("infallible");
        let status = response.status();
        (status, response)
    }

    // Each refusal below rides through with 200 on the unguarded router —
    // the guard is what turns it away.

    #[tokio::test]
    async fn post_from_loopback_passes() {
        let (status, _) = status_of(request(Method::POST, "/mcp/agent1")).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn foreign_peer_refused() {
        let mut request = request(Method::POST, "/mcp/agent1");
        request.extensions_mut().insert(ConnectInfo(LAN));
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rebinding_host_refused() {
        let mut request = request(Method::POST, "/mcp/agent1");
        request.headers_mut().insert(
            HOST,
            "attacker.example.com".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn localhost_subdomain_host_refused() {
        // WebKit resolves *.localhost through system DNS (magpie #2256)
        let mut request = request(Method::POST, "/mcp/agent1");
        request
            .headers_mut()
            .insert(HOST, "app.localhost".parse().expect("static header"));
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn foreign_origin_refused() {
        let mut request = request(Method::POST, "/mcp/agent1");
        request.headers_mut().insert(
            ORIGIN,
            "https://evil.example.com".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn null_origin_refused() {
        let mut request = request(Method::POST, "/mcp/agent1");
        request
            .headers_mut()
            .insert(ORIGIN, "null".parse().expect("static header"));
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn local_origin_passes() {
        let mut request = request(Method::POST, "/mcp/agent1");
        request.headers_mut().insert(
            ORIGIN,
            "http://localhost:5173".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn app_scheme_origin_passes() {
        // desktop webviews/extensions are not CORS-bound; a web page cannot
        // send these schemes
        let mut request = request(Method::POST, "/mcp/agent1");
        request.headers_mut().insert(
            ORIGIN,
            "tauri://localhost".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn get_refused_with_allow_post() {
        let (_, response) = status_of(request(Method::GET, "/mcp/agent1")).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response
                .headers()
                .get(header::ALLOW)
                .and_then(|v| v.to_str().ok()),
            Some("POST")
        );
    }

    #[tokio::test]
    async fn management_route_skips_origin_and_method_guards() {
        // loopback peer, foreign origin, GET — /projection still serves
        let mut request = request(Method::GET, "/projection");
        request.headers_mut().insert(
            ORIGIN,
            "https://evil.example.com".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn management_route_still_guards_host() {
        let mut request = request(Method::GET, "/projection");
        request.headers_mut().insert(
            HOST,
            "attacker.example.com".parse().expect("static header"),
        );
        let (status, _) = status_of(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rate_limit_refuses_before_execution() {
        let limiter = Arc::new(RateLimiter::new());
        let layer_limiter = Arc::clone(&limiter);
        let router = axum::Router::new()
            .route("/mcp/agent1", post(|| async { "mcp" }))
            .layer(axum::middleware::from_fn(move |request, next| {
                guard_chain("/mcp".to_string(), Arc::clone(&layer_limiter), request, next)
            }));
        // drain the 8 MiB bucket with 2 MiB posts, then expect the JSON-RPC
        // rate_limited refusal
        let mut last = None;
        for _ in 0..5 {
            let mut request = request(Method::POST, "/mcp/agent1");
            request
                .headers_mut()
                .insert(header::CONTENT_LENGTH, "2097152".parse().expect("static"));
            last = Some(router.clone().oneshot(request).await.expect("infallible"));
        }
        let response = last.expect("loop ran");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("small body");
        let parsed: Value = serde_json::from_slice(&body).expect("json body");
        assert_eq!(parsed["error"]["data"]["code"], "rate_limited");
        assert_eq!(parsed["error"]["data"]["retryable"], true);
    }

    #[tokio::test]
    async fn ipv6_bracketed_origin_passes() {
        assert!(!origin_refused(Some("http://[::1]:8080")));
    }

    // ---- pure classification fns ----

    #[test]
    fn host_token_strips_ports() {
        assert_eq!(host_token("localhost:2893"), "localhost");
        assert_eq!(host_token("127.0.0.1:8080"), "127.0.0.1");
        assert_eq!(host_token("[::1]:9090"), "::1");
        assert_eq!(host_token("::1"), "::1");
        assert_eq!(host_token("evil.example.com:80"), "evil.example.com");
    }

    #[test]
    fn host_check_accepts_loopback_names_only() {
        assert!(host_is_loopback(Some("localhost:2893")));
        assert!(host_is_loopback(Some("127.0.0.1:8080")));
        assert!(host_is_loopback(Some("[::1]:9090")));
        assert!(host_is_loopback(Some("127.0.0.1"))); // no port
        assert!(!host_is_loopback(Some("attacker.example.com")));
        assert!(!host_is_loopback(Some("attacker.localhost")));
        assert!(!host_is_loopback(Some("127.0.0.1.evil.com")));
    }

    #[test]
    fn origin_classification_matches_magpie() {
        assert!(!origin_refused(None));
        assert!(!origin_refused(Some("")));
        assert!(origin_refused(Some("null")));
        assert!(origin_refused(Some("NULL")));
        assert!(!origin_refused(Some("tauri://localhost")));
        assert!(!origin_refused(Some("vscode-webview://webview")));
        assert!(!origin_refused(Some("chrome-extension://abcdef")));
        assert!(!origin_refused(Some("http://localhost:5173")));
        assert!(!origin_refused(Some("http://127.0.0.1:3000")));
        assert!(!origin_refused(Some("http://[::1]:8080")));
        assert!(origin_refused(Some("https://evil.example.com")));
        assert!(origin_refused(Some("http://app.localhost:5173"))); // #2256
        assert!(!origin_refused(Some("gibberish"))); // not http-ish junk
        assert!(origin_refused(Some("http://gibberish"))); // http-ish junk
    }
}
