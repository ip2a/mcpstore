use std::{
    borrow::Cow,
    collections::HashMap,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};

use futures::{stream::BoxStream, Stream, StreamExt, TryStreamExt};
use http::{
    header::{RETRY_AFTER, WWW_AUTHENTICATE},
    HeaderMap, HeaderName, HeaderValue, StatusCode,
};
use reqwest::header::ACCEPT;
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, ErrorData, JsonRpcMessage, ServerJsonRpcMessage},
    transport::{
        common::http_header::{EVENT_STREAM_MIME_TYPE, HEADER_SESSION_ID, JSON_MIME_TYPE},
        streamable_http_client::{
            AuthRequiredError, HttpStatusError, InsufficientScopeError, StreamableHttpClient,
            StreamableHttpError, StreamableHttpPostResponse,
        },
    },
};
use sse_stream::{Error as SseError, Sse, SseStream};

use crate::{
    identity::InstanceId,
    telemetry::{
        bind_jsonrpc_trace, trace_for_call_id, trace_for_jsonrpc, CallTrace, CALL_ID_META_KEY,
    },
};

pub(super) const DEFAULT_MAX_SSE_EVENT_SIZE: usize = 16 * 1024 * 1024;
const MAX_JSON_RESPONSE_SIZE: usize = 16 * 1024 * 1024;
const MAX_ERROR_RESPONSE_SIZE: usize = 64 * 1024;
const MAX_RETRY_AFTER_BYTES: usize = 128;
const JSON_RPC_BODY_PREVIEW_LEN: usize = 256;

/// Reqwest backend with JSON-RPC correlation established before the POST starts.
/// Request-body socket writes are not observable through reqwest, so this client
/// deliberately leaves `request_write_ms` unset.
#[derive(Clone)]
pub(super) struct ObservedHttpClient {
    client: reqwest::Client,
    instance_id: Arc<str>,
}

impl ObservedHttpClient {
    pub(super) fn new(client: reqwest::Client, instance_id: InstanceId) -> Self {
        Self {
            client,
            instance_id: Arc::from(instance_id.to_string()),
        }
    }

    fn bind_request_trace(&self, message: &ClientJsonRpcMessage) -> Option<Arc<CallTrace>> {
        let envelope = serde_json::to_value(message).ok()?;
        let call_id = envelope
            .pointer(&format!(
                "/params/_meta/{}",
                CALL_ID_META_KEY.replace('/', "~1")
            ))?
            .as_str()?;
        let trace = trace_for_call_id(call_id)?;
        let id = envelope.get("id").map(jsonrpc_id)?;
        bind_jsonrpc_trace(&self.instance_id, id, Arc::clone(&trace));
        Some(trace)
    }

    async fn post_message_observed(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<reqwest::Error>> {
        let trace = self.bind_request_trace(&message);
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "));
        if let Some(auth_header) = auth_token {
            request = request.bearer_auth(auth_header);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let session_was_attached = session_id.is_some();
        if let Some(session_id) = session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }

        // Reqwest resolves send() when response headers are available. This is
        // the HTTP first-byte boundary; the body has not necessarily been read.
        let response = request.json(&message).send().await?;
        if let Some(trace) = trace {
            trace.mark_first_byte();
        }

        if response.status() == StatusCode::UNAUTHORIZED {
            if let Some(header) = response.headers().get(WWW_AUTHENTICATE) {
                let header = header
                    .to_str()
                    .map_err(|_| {
                        StreamableHttpError::UnexpectedServerResponse(Cow::from(
                            "invalid www-authenticate header value",
                        ))
                    })?
                    .to_string();
                return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
                    header,
                )));
            }
        }
        if response.status() == StatusCode::FORBIDDEN {
            if let Some(header) = response.headers().get(WWW_AUTHENTICATE) {
                let header_str = header.to_str().map_err(|_| {
                    StreamableHttpError::UnexpectedServerResponse(Cow::from(
                        "invalid www-authenticate header value",
                    ))
                })?;
                let scope = extract_scope_from_header(header_str);
                return Err(StreamableHttpError::InsufficientScope(
                    InsufficientScopeError::new(header_str.to_string(), scope),
                ));
            }
        }

        let status = response.status();
        if matches!(status, StatusCode::ACCEPTED | StatusCode::NO_CONTENT) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == StatusCode::NOT_FOUND && session_was_attached {
            return Err(StreamableHttpError::SessionExpired);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).to_string());
        let is_json_response = content_type
            .as_deref()
            .is_some_and(|value| value.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()));
        let status_error = match (
            http_status_error(status, response.headers()),
            is_json_response,
        ) {
            (Some(error), is_json) if !is_json || status == StatusCode::TOO_MANY_REQUESTS => {
                return Err(error.into());
            }
            (error, _) => error,
        };
        let content_length = response.content_length();
        let session_id = response
            .headers()
            .get(HEADER_SESSION_ID)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if status.is_success()
            && content_length == Some(0)
            && matches!(
                message,
                ClientJsonRpcMessage::Notification(_)
                    | ClientJsonRpcMessage::Response(_)
                    | ClientJsonRpcMessage::Error(_)
            )
        {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        if !status.is_success() {
            let content_type_header = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .cloned();
            let body = match read_bounded_body(
                response.bytes_stream(),
                content_length,
                MAX_ERROR_RESPONSE_SIZE,
            )
            .await
            {
                Ok(body) => {
                    let mut buffered = http::Response::new(body);
                    if let Some(content_type) = content_type_header {
                        buffered
                            .headers_mut()
                            .insert(reqwest::header::CONTENT_TYPE, content_type);
                    }
                    reqwest::Response::from(buffered)
                        .text()
                        .await
                        .unwrap_or_else(|_| "<failed to read response body>".to_owned())
                }
                Err(error @ StreamableHttpError::ResponseBodyTooLarge { .. }) => {
                    return Err(status_error.map(StreamableHttpError::from).unwrap_or(error));
                }
                Err(_) => "<failed to read response body>".to_owned(),
            };
            if let Some(response) =
                legacy_discover_response(&message, session_was_attached, status, &body)
            {
                return Ok(response);
            }
            let is_auth_failure =
                matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN);
            if !is_auth_failure && is_json_response {
                match parse_json_rpc_error(&body) {
                    Some(message) => {
                        mark_response_read(&self.instance_id, &message);
                        return Ok(StreamableHttpPostResponse::Json(message, session_id));
                    }
                    None => tracing::warn!(
                        "HTTP {status}: could not parse JSON body as a JSON-RPC error"
                    ),
                }
            }
            if let Some(error) = status_error {
                return Err(error.into());
            }
            return Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                format!("HTTP {status}: {body}"),
            )));
        }

        match content_type.as_deref() {
            Some(value)
                if value
                    .as_bytes()
                    .starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) =>
            {
                Ok(StreamableHttpPostResponse::Sse(
                    observed_sse_stream(
                        response,
                        max_sse_event_size,
                        Arc::clone(&self.instance_id),
                    ),
                    session_id,
                ))
            }
            Some(value) if value.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()) => {
                let body = read_bounded_body(
                    response.bytes_stream(),
                    content_length,
                    MAX_JSON_RESPONSE_SIZE,
                )
                .await?;
                match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                    Ok(parsed) => {
                        mark_response_read(&self.instance_id, &parsed);
                        Ok(StreamableHttpPostResponse::Json(parsed, session_id))
                    }
                    Err(error)
                        if matches!(
                            message,
                            ClientJsonRpcMessage::Notification(_)
                                | ClientJsonRpcMessage::Response(_)
                                | ClientJsonRpcMessage::Error(_)
                        ) =>
                    {
                        tracing::warn!(
                            "could not parse JSON response as ServerJsonRpcMessage, treating as accepted: {error}"
                        );
                        Ok(StreamableHttpPostResponse::Accepted)
                    }
                    Err(error) => Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                        json_rpc_parse_error_message(&error, &body),
                    ))),
                }
            }
            _ => {
                tracing::error!("unexpected content type: {:?}", content_type);
                Err(StreamableHttpError::UnexpectedContentType(content_type))
            }
        }
    }
}

impl StreamableHttpClient for ObservedHttpClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_observed(
            uri,
            message,
            session_id,
            auth_header,
            custom_headers,
            DEFAULT_MAX_SSE_EVENT_SIZE,
        )
        .await
    }

    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_observed(
            uri,
            message,
            session_id,
            auth_header,
            custom_headers,
            max_sse_event_size,
        )
        .await
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.client
            .delete_session(uri, session_id, auth_header, custom_headers)
            .await
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        self.client
            .get_stream(uri, session_id, last_event_id, auth_header, custom_headers)
            .await
    }

    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        self.client
            .get_stream_with_max_sse_event_size(
                uri,
                session_id,
                last_event_id,
                auth_header,
                custom_headers,
                max_sse_event_size,
            )
            .await
    }
}

fn jsonrpc_id(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(id) => id.clone(),
        other => other.to_string(),
    }
}

fn mark_response_read(instance_id: &str, message: &ServerJsonRpcMessage) {
    let id = match message {
        ServerJsonRpcMessage::Response(response) => Some(&response.id),
        ServerJsonRpcMessage::Error(error) => error.id.as_ref(),
        _ => None,
    };
    let Some(id) = id else {
        return;
    };
    let Ok(id) = serde_json::to_value(id) else {
        return;
    };
    if let Some(trace) = trace_for_jsonrpc(instance_id, &jsonrpc_id(&id)) {
        trace.mark_response_read();
    }
}

fn apply_custom_headers(
    mut builder: reqwest::RequestBuilder,
    custom_headers: HashMap<HeaderName, HeaderValue>,
) -> Result<reqwest::RequestBuilder, StreamableHttpError<reqwest::Error>> {
    for (name, value) in custom_headers {
        validate_custom_header(&name).map_err(StreamableHttpError::ReservedHeaderConflict)?;
        builder = builder.header(name, value);
    }
    Ok(builder)
}

fn validate_custom_header(name: &HeaderName) -> Result<(), String> {
    const RESERVED_HEADERS: &[&str] = &[
        "accept",
        HEADER_SESSION_ID,
        "MCP-Protocol-Version",
        "Last-Event-Id",
    ];
    if RESERVED_HEADERS
        .iter()
        .any(|reserved| name.as_str().eq_ignore_ascii_case(reserved))
    {
        if name.as_str().eq_ignore_ascii_case("MCP-Protocol-Version") {
            return Ok(());
        }
        return Err(name.to_string());
    }
    Ok(())
}

fn extract_scope_from_header(header: &str) -> Option<String> {
    let header_lowercase = header.to_ascii_lowercase();
    let scope_key = "scope=";
    if let Some(position) = header_lowercase.find(scope_key) {
        let start = position + scope_key.len();
        let value = &header[start..];
        if let Some(stripped) = value.strip_prefix('"') {
            if let Some(end_quote) = stripped.find('"') {
                return Some(stripped[..end_quote].to_string());
            }
        } else {
            let end = value
                .find(|character: char| {
                    character == ',' || character == ';' || character.is_whitespace()
                })
                .unwrap_or(value.len());
            if end > 0 {
                return Some(value[..end].to_string());
            }
        }
    }
    None
}

fn parse_json_rpc_error(body: &str) -> Option<ServerJsonRpcMessage> {
    match serde_json::from_str::<ServerJsonRpcMessage>(body) {
        Ok(message @ JsonRpcMessage::Error(_)) => Some(message),
        _ => None,
    }
}

fn http_status_error(status: StatusCode, headers: &HeaderMap) -> Option<HttpStatusError> {
    if status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error() {
        return None;
    }
    let retry_after = headers
        .get(RETRY_AFTER)
        .filter(|value| value.as_bytes().len() <= MAX_RETRY_AFTER_BYTES)
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok())
        .map(str::to_owned);
    Some(HttpStatusError::new(status.as_u16(), retry_after))
}

async fn read_bounded_body<S, D, E>(
    stream: S,
    content_length: Option<u64>,
    max_size: usize,
) -> Result<Vec<u8>, StreamableHttpError<E>>
where
    S: Stream<Item = Result<D, E>>,
    D: AsRef<[u8]>,
    E: std::error::Error + Send + Sync + 'static,
{
    if content_length.is_some_and(|length| length > max_size as u64) {
        return Err(StreamableHttpError::ResponseBodyTooLarge { max_size });
    }
    futures::pin_mut!(stream);
    let mut body = Vec::new();
    while let Some(chunk) = stream
        .try_next()
        .await
        .map_err(StreamableHttpError::Client)?
    {
        let chunk = chunk.as_ref();
        if chunk.len() > max_size - body.len() {
            return Err(StreamableHttpError::ResponseBodyTooLarge { max_size });
        }
        body.extend_from_slice(chunk);
    }
    Ok(body)
}

fn json_rpc_parse_error_message(error: &serde_json::Error, body: &[u8]) -> String {
    format!(
        "could not parse JSON response as ServerJsonRpcMessage: {error}: {}",
        json_rpc_body_preview(body)
    )
}

fn json_rpc_body_preview(body: &[u8]) -> Cow<'static, str> {
    if body.is_empty() {
        return Cow::Borrowed("<empty>");
    }
    let lossy = String::from_utf8_lossy(body);
    if lossy.len() <= JSON_RPC_BODY_PREVIEW_LEN {
        return Cow::Owned(lossy.into_owned());
    }
    let mut end = JSON_RPC_BODY_PREVIEW_LEN;
    while end > 0 && !lossy.is_char_boundary(end) {
        end -= 1;
    }
    Cow::Owned(lossy[..end].to_owned())
}

fn legacy_discover_response(
    message: &ClientJsonRpcMessage,
    session_was_attached: bool,
    status: StatusCode,
    body: &str,
) -> Option<StreamableHttpPostResponse> {
    if session_was_attached
        || !status.is_client_error()
        || matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
    {
        return None;
    }
    let ClientJsonRpcMessage::Request(request) = message else {
        return None;
    };
    if !matches!(request.request, ClientRequest::DiscoverRequest(_)) {
        return None;
    }
    let error = match serde_json::from_str::<ServerJsonRpcMessage>(body) {
        Ok(ServerJsonRpcMessage::Error(error)) => error.error,
        _ => ErrorData::invalid_request(
            format!("server/discover rejected with HTTP {status}: {body}"),
            None,
        ),
    };
    Some(StreamableHttpPostResponse::Json(
        ServerJsonRpcMessage::error(error, Some(request.id.clone())),
        None,
    ))
}

#[derive(Debug, thiserror::Error)]
enum BoundedSseStreamError {
    #[error(transparent)]
    Source(Box<dyn std::error::Error + Send + Sync>),
    #[error("SSE event exceeded the maximum size of {max_size} bytes")]
    EventTooLarge { max_size: usize },
}

#[derive(Debug)]
struct SseEventSizeLimiter {
    max_size: usize,
    retained_size: usize,
    line_size: usize,
    line_is_comment: bool,
    previous_was_cr: bool,
}

impl SseEventSizeLimiter {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            retained_size: 0,
            line_size: 0,
            line_is_comment: false,
            previous_was_cr: false,
        }
    }

    fn observe(&mut self, chunk: &[u8]) -> Result<(), ()> {
        for &byte in chunk {
            if self.previous_was_cr {
                self.previous_was_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => {
                    self.finish_line()?;
                    self.previous_was_cr = true;
                }
                b'\n' => self.finish_line()?,
                _ => {
                    if self.line_size == 0 {
                        self.line_is_comment = byte == b':';
                    }
                    self.line_size = self.line_size.saturating_add(1);
                    self.check_limit()?;
                }
            }
        }
        Ok(())
    }

    fn finish_line(&mut self) -> Result<(), ()> {
        if self.line_size == 0 {
            self.retained_size = 0;
        } else if !self.line_is_comment {
            self.retained_size = self
                .retained_size
                .saturating_add(self.line_size)
                .saturating_add(1);
        }
        self.line_size = 0;
        self.line_is_comment = false;
        self.check_limit()
    }

    fn check_limit(&self) -> Result<(), ()> {
        if self.retained_size.saturating_add(self.line_size) > self.max_size {
            Err(())
        } else {
            Ok(())
        }
    }
}

struct BoundedSseByteStream<S, D, E> {
    inner: Pin<Box<S>>,
    limiter: SseEventSizeLimiter,
    failed: bool,
    marker: PhantomData<fn() -> (D, E)>,
}

impl<S, D, E> Stream for BoundedSseByteStream<S, D, E>
where
    S: Stream<Item = Result<D, E>>,
    D: AsRef<[u8]>,
    E: std::error::Error + Send + Sync + 'static,
{
    type Item = Result<D, BoundedSseStreamError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.failed {
            return Poll::Ready(None);
        }
        match ready!(this.inner.as_mut().poll_next(context)) {
            Some(Ok(chunk)) => {
                if this.limiter.observe(chunk.as_ref()).is_err() {
                    this.failed = true;
                    Poll::Ready(Some(Err(BoundedSseStreamError::EventTooLarge {
                        max_size: this.limiter.max_size,
                    })))
                } else {
                    Poll::Ready(Some(Ok(chunk)))
                }
            }
            Some(Err(error)) => {
                this.failed = true;
                Poll::Ready(Some(Err(BoundedSseStreamError::Source(Box::new(error)))))
            }
            None => Poll::Ready(None),
        }
    }
}

fn observed_sse_stream(
    response: reqwest::Response,
    max_event_size: usize,
    instance_id: Arc<str>,
) -> BoxStream<'static, Result<Sse, SseError>> {
    let stream = BoundedSseByteStream {
        inner: Box::pin(response.bytes_stream()),
        limiter: SseEventSizeLimiter::new(max_event_size),
        failed: false,
        marker: PhantomData,
    };
    SseStream::from_bytes_stream(stream)
        .inspect(move |event| {
            let Ok(event) = event else {
                return;
            };
            if !matches!(event.event.as_deref(), None | Some("") | Some("message")) {
                return;
            }
            let Some(data) = event.data.as_deref() else {
                return;
            };
            if let Ok(message) = serde_json::from_str::<ServerJsonRpcMessage>(data) {
                mark_response_read(&instance_id, &message);
            }
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    use super::*;

    fn instance(name: &str) -> InstanceId {
        crate::identity::ServiceInstanceKey::new(name, crate::ScopeRef::Store).instance_id()
    }

    fn ping_message() -> ClientJsonRpcMessage {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "ping", "params": null
        }))
        .unwrap()
    }

    fn call_message(call_id: &str) -> ClientJsonRpcMessage {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "test",
                "arguments": {},
                "_meta": { (CALL_ID_META_KEY): call_id }
            }
        }))
        .unwrap()
    }

    async fn spawn_response(
        status: &'static str,
        content_type: &'static str,
        content_length: usize,
        chunks: Vec<(Duration, Vec<u8>)>,
    ) -> (Arc<str>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut buffer).await.unwrap();
                assert_ne!(read, 0, "client closed before sending request headers");
                request.extend_from_slice(&buffer[..read]);
            }
            let headers = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
            for (delay, chunk) in chunks {
                tokio::time::sleep(delay).await;
                socket.write_all(&chunk).await.unwrap();
                socket.flush().await.unwrap();
            }
        });
        (Arc::from(format!("http://{address}/mcp")), server)
    }

    #[tokio::test]
    async fn json_first_byte_waits_for_headers_not_full_body() {
        let body = br#"{"jsonrpc":"2.0","id":7,"result":{}}"#.to_vec();
        let (uri, server) = spawn_response(
            "200 OK",
            "application/json",
            body.len(),
            vec![(Duration::from_millis(120), body)],
        )
        .await;
        let instance_id = instance("observed-http-json-timing");
        let trace = CallTrace::register("observed-http-json-timing");
        let client = ObservedHttpClient::new(reqwest::Client::new(), instance_id);

        let response = client
            .post_message(
                uri,
                call_message(trace.call_id()),
                None,
                None,
                HashMap::new(),
            )
            .await
            .unwrap();
        server.await.unwrap();
        assert!(matches!(response, StreamableHttpPostResponse::Json(..)));

        let timing = trace.timing();
        assert_eq!(timing.request_write_ms, None);
        let first_byte = timing.first_byte_ms.unwrap();
        let response_read = timing.response_read_ms.unwrap();
        assert!(
            response_read >= first_byte + 50,
            "headers and full body must remain distinct: {timing:?}"
        );
        assert!(trace_for_jsonrpc(&instance_id.to_string(), "7").is_some());
        trace.unregister();
    }

    #[tokio::test]
    async fn json_and_error_bodies_keep_rmcp_size_limits() {
        for (status, expected_limit) in [
            ("200 OK", MAX_JSON_RESPONSE_SIZE),
            ("400 Bad Request", MAX_ERROR_RESPONSE_SIZE),
        ] {
            let (uri, server) =
                spawn_response(status, "application/json", expected_limit + 1, Vec::new()).await;
            let client = ObservedHttpClient::new(
                reqwest::Client::new(),
                instance("observed-http-size-limits"),
            );
            let result = client
                .post_message(uri, ping_message(), None, None, HashMap::new())
                .await;
            server.await.unwrap();
            assert!(
                matches!(
                    result,
                    Err(StreamableHttpError::ResponseBodyTooLarge { max_size })
                        if max_size == expected_limit
                ),
                "unexpected result for {status}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn streamed_body_limit_ignores_undersized_length_hint() {
        let chunks =
            [b"abcd".to_vec(), b"efgh".to_vec()].map(|chunk| Ok::<_, std::io::Error>(chunk));
        let error = read_bounded_body(futures::stream::iter(chunks), Some(1), 7)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            StreamableHttpError::ResponseBodyTooLarge { max_size: 7 }
        ));
    }

    #[tokio::test]
    async fn sse_response_read_waits_for_matching_message_event() {
        let event = serde_json::json!({"jsonrpc":"2.0","id":7,"result":{}});
        let body = format!("data: {event}\n\n").into_bytes();
        let (uri, server) = spawn_response(
            "200 OK",
            "text/event-stream",
            body.len(),
            vec![(Duration::ZERO, body)],
        )
        .await;
        let trace = CallTrace::register("observed-http-sse-timing");
        let client =
            ObservedHttpClient::new(reqwest::Client::new(), instance("observed-http-sse-timing"));

        let response = client
            .post_message(
                uri,
                call_message(trace.call_id()),
                None,
                None,
                HashMap::new(),
            )
            .await
            .unwrap();
        let StreamableHttpPostResponse::Sse(mut stream, _) = response else {
            panic!("expected SSE response");
        };
        assert!(trace.timing().first_byte_ms.is_some());
        assert_eq!(trace.timing().response_read_ms, None);

        stream.next().await.unwrap().unwrap();
        server.await.unwrap();
        assert!(trace.timing().response_read_ms.is_some());
        assert_eq!(trace.timing().request_write_ms, None);
        trace.unregister();
    }

    #[tokio::test]
    async fn sse_response_enforces_configured_raw_event_limit() {
        let body = format!("data: {}\n\n", "x".repeat(64)).into_bytes();
        let (uri, server) = spawn_response(
            "200 OK",
            "text/event-stream",
            body.len(),
            vec![(Duration::ZERO, body)],
        )
        .await;
        let client =
            ObservedHttpClient::new(reqwest::Client::new(), instance("observed-http-sse-limit"));

        let response = client
            .post_message_with_max_sse_event_size(
                uri,
                ping_message(),
                None,
                None,
                HashMap::new(),
                16,
            )
            .await
            .unwrap();
        let StreamableHttpPostResponse::Sse(mut stream, _) = response else {
            panic!("expected SSE response");
        };
        let error = stream.next().await.unwrap().unwrap_err();
        server.await.unwrap();
        assert!(error.to_string().contains("maximum size of 16 bytes"));
    }
}
