use super::*;
use axum::Router;
use base64::Engine;
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServerHandler,
};
use serde_json::Map;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor};
use crate::identity::{InstanceId, ScopeRef, ServiceInstanceKey};

fn temp_config_path() -> String {
    std::env::temp_dir()
        .join(format!("mcpstore-store-{}.json", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string()
}

fn store_scope() -> ScopeRef {
    ScopeRef::Store
}

fn agent_scope(agent_id: &str) -> ScopeRef {
    ScopeRef::Agent {
        agent_id: agent_id.to_string(),
    }
}

fn instance_id(service_name: &str, scope: ScopeRef) -> InstanceId {
    ServiceInstanceKey::new(service_name, scope).instance_id()
}

fn store_instance_id(service_name: &str) -> InstanceId {
    instance_id(service_name, store_scope())
}

fn write_temp_file(contents: &str) -> String {
    let path = std::env::temp_dir().join(format!("mcpstore-file-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&path, contents).unwrap();
    path.to_string_lossy().to_string()
}

fn stdio_config() -> ServerConfig {
    ServerConfig {
        url: None,
        command: Some("echo".to_string()),
        args: vec!["fixture".to_string()],
        env: HashMap::new(),
        headers: HashMap::new(),
        auth: Default::default(),
        transport: Some("stdio".to_string()),
        working_dir: None,
        description: Some("fixture".to_string()),
        mcpstore: None,
        extra: Map::new(),
    }
}

fn oauth_http_config() -> ServerConfig {
    let mut config = stdio_config();
    config.command = None;
    config.args.clear();
    config.url = Some("https://mcp.example/protected".to_string());
    config.transport = Some("streamable-http".to_string());
    config.auth = serde_json::from_value(serde_json::json!({
        "type": "oauth_authorization_code",
        "client_id": "client-1",
        "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
        "scopes": ["tools.read"]
    }))
    .unwrap();
    config
}

fn broken_stdio_config() -> ServerConfig {
    ServerConfig {
        url: None,
        command: Some("__mcpstore_missing_binary__".to_string()),
        args: Vec::new(),
        env: HashMap::new(),
        headers: HashMap::new(),
        auth: Default::default(),
        transport: Some("stdio".to_string()),
        working_dir: None,
        description: Some("broken".to_string()),
        mcpstore: None,
        extra: Map::new(),
    }
}

fn hanging_stdio_config() -> ServerConfig {
    ServerConfig {
        url: None,
        command: Some("sh".to_string()),
        args: vec!["-c".to_string(), "sleep 60".to_string()],
        env: HashMap::new(),
        headers: HashMap::new(),
        auth: Default::default(),
        transport: Some("stdio".to_string()),
        working_dir: None,
        description: Some("hanging".to_string()),
        mcpstore: None,
        extra: Map::new(),
    }
}

fn stdio_config_with_lifecycle(
    startup_policy: Option<crate::config::StartupPolicy>,
    restart_policy: Option<crate::config::RestartPolicy>,
) -> ServerConfig {
    config_with_lifecycle(stdio_config(), startup_policy, restart_policy)
}

fn config_with_lifecycle(
    mut config: ServerConfig,
    startup_policy: Option<crate::config::StartupPolicy>,
    restart_policy: Option<crate::config::RestartPolicy>,
) -> ServerConfig {
    config.mcpstore = Some(crate::config::McpStoreExtension {
        scopes: ScopeDeclarations::store_only(),
        lifecycle: Some(crate::config::ServiceLifecycleConfig {
            startup_policy,
            restart_policy,
        }),
        revision: 1,
        extra: Map::new(),
    });
    config
}

fn broken_stdio_config_with_restart_policy(policy: crate::config::RestartPolicy) -> ServerConfig {
    config_with_lifecycle(broken_stdio_config(), None, Some(policy))
}

async fn spawn_openapi_http_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let request_lower = request.to_ascii_lowercase();
                let json_bytes = |value: serde_json::Value| value.to_string().into_bytes();
                let (status, body, content_type) = if first_line.starts_with("GET /items ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({"items": ["apple", "pear"]})),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /items/sku-1 ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({"sku": "sku-1", "name": "apple"})),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /plain ") {
                    (
                        "200 OK",
                        b"plain inventory".to_vec(),
                        "text/plain; charset=utf-8",
                    )
                } else if first_line.starts_with("GET /document ") {
                    ("200 OK", b"%PDF fixture".to_vec(), "application/pdf")
                } else if first_line.starts_with("GET /profile ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({
                            "id": "profile-1",
                            "username": "alice",
                            "password": "server-secret",
                            "tokens": ["token-1", "token-2"],
                            "metadata": { "public": "visible", "secret": "hidden" }
                        })),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /invalid-profile ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({
                            "id": 123,
                            "metadata": {},
                            "tags": []
                        })),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /negotiated ") {
                    if request_lower.contains("accept: ")
                        && request_lower.contains("application/json")
                        && request_lower.contains("text/plain")
                        && request_lower.contains("image/png")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "accept"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad accept header"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /image ") {
                    ("200 OK", b"png-bytes".to_vec(), "image/png")
                } else if first_line.starts_with("POST /items ")
                    || first_line.starts_with("POST /items?")
                {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({"created": true, "path": "/items"})),
                        "application/json",
                    )
                } else if first_line.starts_with("POST /profile ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({
                            "id": "profile-1",
                            "username": "alice",
                            "password": "server-secret",
                            "tokens": ["token-1", "token-2"],
                            "metadata": { "public": "visible", "secret": "hidden" }
                        })),
                        "application/json",
                    )
                } else if first_line.starts_with("POST /invalid-profile ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({
                            "id": 123,
                            "metadata": {},
                            "tags": []
                        })),
                        "application/json",
                    )
                } else if first_line.starts_with("POST /forms/urlencoded ") {
                    if request_lower.contains("content-type: application/x-www-form-urlencoded")
                        && request.contains("name=apple")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "urlencoded"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad urlencoded body"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /forms/multipart ") {
                    if request_lower.contains("content-type: multipart/form-data")
                        && request.contains("name=\"name\"")
                        && request.contains("apple")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "multipart"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad multipart body"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /forms/file ") {
                    if request_lower.contains("content-type: multipart/form-data")
                        && request.contains("name=\"file\"; filename=\"apple.txt\"")
                        && request_lower.contains("content-type: text/plain")
                        && request.contains("apple-bytes")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "file"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad multipart file body"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /forms/files ") {
                    if request_lower.contains("content-type: multipart/form-data")
                        && request.contains("name=\"files\"; filename=\"apple.txt\"")
                        && request.contains("name=\"files\"; filename=\"pear.txt\"")
                        && request.contains("apple-bytes")
                        && request.contains("pear-bytes")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "files"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad multipart files body"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /forms/text ") {
                    if request_lower.contains("content-type: text/plain")
                        && request.contains("plain-body")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "text"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": "bad text body"})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /search/a,b?") {
                    if first_line.contains("tag=red")
                        && first_line.contains("tag=blue")
                        && first_line.contains("compact=one%2Ctwo")
                        && request_lower.contains("x-flags: fast,safe")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "parameters"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": first_line})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /reserved?") {
                    if first_line.contains("raw=https://example.com/a,b;c=1")
                        && first_line.contains("encoded=https%3A%2F%2Fexample.com%2Fa%2Cb%3Bc%3D1")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "reserved"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": first_line})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /deep?") {
                    if first_line.contains("filter%5Bcolor%5D=red")
                        && first_line.contains("filter%5Bsize%5D=large")
                    {
                        (
                            "200 OK",
                            json_bytes(serde_json::json!({"received": "deep-object"})),
                            "application/json",
                        )
                    } else {
                        (
                            "404 Not Found",
                            json_bytes(serde_json::json!({"error": first_line})),
                            "application/json",
                        )
                    }
                } else if first_line.starts_with("POST /styled/.red.blue/;id=sku-1;id=sku-2 ") {
                    (
                        "200 OK",
                        json_bytes(serde_json::json!({"received": "styled-path"})),
                        "application/json",
                    )
                } else {
                    (
                        "404 Not Found",
                        json_bytes(serde_json::json!({"error": first_line})),
                        "application/json",
                    )
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    format!("http://{addr}")
}

async fn spawn_openapi_slow_http_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buffer = vec![0; 1024];
        let _ = socket.read(&mut buffer).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        let body = br#"{"ok":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.write_all(body).await;
    });
    format!("http://{addr}")
}

async fn spawn_openapi_auth_http_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let authorized = request.lines().any(|line| {
                    line.split_once(':')
                        .map(|(name, value)| {
                            name.eq_ignore_ascii_case("x-api-key") && value.trim() == "secret"
                        })
                        .unwrap_or(false)
                });
                let (status, body) = if !authorized {
                    (
                        "401 Unauthorized",
                        serde_json::json!({"error": "missing api key"}).to_string(),
                    )
                } else if first_line.starts_with("GET /secure/items ") {
                    (
                        "200 OK",
                        serde_json::json!({"items": ["secured"]}).to_string(),
                    )
                } else if first_line.starts_with("POST /secure/items ") {
                    ("200 OK", serde_json::json!({"created": true}).to_string())
                } else {
                    (
                        "404 Not Found",
                        serde_json::json!({"error": first_line}).to_string(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

async fn spawn_openapi_spec_ref_fixture() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let components_requests = Arc::new(AtomicUsize::new(0));
    let base_url_for_task = base_url.clone();
    let components_requests_for_task = components_requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let base_url = base_url_for_task.clone();
            let components_requests = components_requests_for_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let (status, body, content_type) = if first_line.starts_with("GET /openapi.json ") {
                    (
                        "200 OK",
                        serde_json::json!({
                            "openapi": "3.0.0",
                            "info": { "title": "External Refs", "version": "2026.1" },
                            "servers": [{ "url": base_url }],
                            "paths": {
                                "/items/{id}": {
                                    "parameters": [{ "$ref": "components.json#/components/parameters/ItemId" }],
                                    "get": {
                                        "operationId": "getItemByExternalRef",
                                        "parameters": [{ "$ref": "components.json#/components/parameters/Verbose" }],
                                        "responses": {
                                            "200": {
                                                "description": "ok",
                                                "content": {
                                                    "application/json": { "schema": { "$ref": "components.json#/components/schemas/Item" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        })
                        .to_string(),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /openapi-yaml-ref.yaml ") {
                    (
                        "200 OK",
                        format!(
                            r#"openapi: 3.0.0
info:
  title: External YAML Refs
  version: '2026.1'
servers:
  - url: {base_url}
paths:
  /items/{{id}}:
    parameters:
      - $ref: components.yaml#/components/parameters/ItemId
    get:
      operationId: getItemByExternalYamlRef
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                $ref: components.yaml#/components/schemas/Item
"#
                        ),
                        "application/yaml",
                    )
                } else if first_line.starts_with("GET /components.json ") {
                    components_requests.fetch_add(1, Ordering::SeqCst);
                    (
                        "200 OK",
                        serde_json::json!({
                            "components": {
                                "parameters": {
                                    "ItemId": {
                                        "name": "id",
                                        "in": "path",
                                        "required": true,
                                        "schema": { "$ref": "#/components/schemas/ItemId" }
                                    },
                                    "Verbose": {
                                        "name": "verbose",
                                        "in": "query",
                                        "schema": { "type": "boolean" }
                                    }
                                },
                                "schemas": {
                                    "ItemId": { "type": "string", "description": "external item id" },
                                    "Item": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "$ref": "#/components/schemas/ItemId" },
                                            "name": { "type": "string" }
                                        }
                                    }
                                }
                            }
                        })
                        .to_string(),
                        "application/json",
                    )
                } else if first_line.starts_with("GET /components.yaml ") {
                    components_requests.fetch_add(1, Ordering::SeqCst);
                    (
                        "200 OK",
                        r#"components:
  parameters:
    ItemId:
      name: id
      in: path
      required: true
      schema:
        $ref: '#/components/schemas/ItemId'
  schemas:
    ItemId:
      type: string
      description: external YAML item id
    Item:
      type: object
      properties:
        id:
          $ref: '#/components/schemas/ItemId'
        name:
          type: string
"#
                        .to_string(),
                        "application/yaml",
                    )
                } else if first_line.starts_with("GET /items/sku-1 ")
                    || first_line.starts_with("GET /items/sku-1?")
                {
                    (
                        "200 OK",
                        serde_json::json!({"id": "sku-1", "name": "apple"}).to_string(),
                        "application/json",
                    )
                } else {
                    (
                        "404 Not Found",
                        serde_json::json!({"error": first_line}).to_string(),
                        "application/json",
                    )
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(body.as_bytes()).await;
            });
        }
    });
    (base_url, components_requests)
}

async fn spawn_openapi_conditional_ref_fixture() -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let components_requests = Arc::new(AtomicUsize::new(0));
    let conditional_requests = Arc::new(AtomicUsize::new(0));
    let base_url_for_task = base_url.clone();
    let components_requests_for_task = components_requests.clone();
    let conditional_requests_for_task = conditional_requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let base_url = base_url_for_task.clone();
            let components_requests = components_requests_for_task.clone();
            let conditional_requests = conditional_requests_for_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                if first_line.starts_with("GET /openapi.json ") {
                    let body = serde_json::json!({
                        "openapi": "3.0.0",
                        "info": { "title": "Conditional External Refs", "version": "2026.1" },
                        "servers": [{ "url": base_url }],
                        "paths": {
                            "/items/{id}": {
                                "get": {
                                    "operationId": "getItemByConditionalExternalRef",
                                    "responses": {
                                        "200": {
                                            "description": "ok",
                                            "content": {
                                                "application/json": { "schema": { "$ref": "components.json#/components/schemas/Item" } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .to_string();
                    let header = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(body.as_bytes()).await;
                    return;
                }

                if first_line.starts_with("GET /components.json ") {
                    components_requests.fetch_add(1, Ordering::SeqCst);
                    if request.contains("if-none-match: \"components-v1\"")
                        || request.contains("If-None-Match: \"components-v1\"")
                    {
                        conditional_requests.fetch_add(1, Ordering::SeqCst);
                        let response = "HTTP/1.1 304 Not Modified\r\netag: \"components-v1\"\r\nlast-modified: Tue, 01 Jan 2030 00:00:00 GMT\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                        let _ = socket.write_all(response.as_bytes()).await;
                        return;
                    }

                    let body = serde_json::json!({
                        "components": {
                            "schemas": {
                                "ItemId": { "type": "string", "description": "conditional external item id" },
                                "Item": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "$ref": "#/components/schemas/ItemId" }
                                    }
                                }
                            }
                        }
                    })
                    .to_string();
                    let header = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\netag: \"components-v1\"\r\nlast-modified: Tue, 01 Jan 2030 00:00:00 GMT\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(body.as_bytes()).await;
                    return;
                }

                let body = serde_json::json!({"error": first_line}).to_string();
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    (base_url, components_requests, conditional_requests)
}

async fn spawn_openapi_yaml_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let base_url_for_task = base_url.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let base_url = base_url_for_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let (status, body, content_type) = if first_line.starts_with("GET /openapi.yaml ") {
                    (
                        "200 OK",
                        format!(
                            r#"openapi: 3.0.0
info:
  title: YAML Inventory
  version: '2026.1'
servers:
  - url: {base_url}
paths:
  /items:
    get:
      operationId: listYamlItems
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                type: object
"#
                        )
                        .into_bytes(),
                        "application/yaml",
                    )
                } else if first_line.starts_with("GET /items ") {
                    (
                        "200 OK",
                        serde_json::json!({"items": ["yaml"]})
                            .to_string()
                            .into_bytes(),
                        "application/json",
                    )
                } else {
                    (
                        "404 Not Found",
                        serde_json::json!({"error": first_line})
                            .to_string()
                            .into_bytes(),
                        "application/json",
                    )
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    base_url
}

async fn copy_cache_snapshot(source: &MCPStore, target: &MCPStore) {
    let snapshot = source.cache().snapshot().await.unwrap();
    for (entity_type, entries) in snapshot.entities {
        for (key, value) in entries {
            target
                .cache()
                .put_entity(&entity_type, &key, value)
                .await
                .unwrap();
        }
    }
    for (relation_type, entries) in snapshot.relations {
        for (key, value) in entries {
            target
                .cache()
                .put_relation(&relation_type, &key, value)
                .await
                .unwrap();
        }
    }
    for (state_type, entries) in snapshot.states {
        for (key, value) in entries {
            target
                .cache()
                .put_state(&state_type, &key, value)
                .await
                .unwrap();
        }
    }
    for (event_type, entries) in snapshot.events {
        for (key, value) in entries {
            target
                .cache()
                .put_event(&event_type, &key, value)
                .await
                .unwrap();
        }
    }
}

fn agent_only_config(agent_id: &str) -> ServerConfig {
    let mut config = stdio_config();
    config.mcpstore = Some(McpStoreExtension {
        scopes: ScopeDeclarations {
            store: None,
            agents: HashMap::from([(agent_id.to_string(), ScopeDescriptor::default())]),
        },
        lifecycle: None,
        revision: 1,
        extra: Map::new(),
    });
    config
}

#[tokio::test]
async fn add_service_writes_definition_and_instance_cache_layers() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");

    assert!(store
        .cache()
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_some());
    assert!(store
        .cache()
        .get_entity("service_instances", &instance_id.to_string())
        .await
        .unwrap()
        .is_some());
    assert!(store
        .cache()
        .get_state("service_state", &instance_id.to_string())
        .await
        .unwrap()
        .is_some());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn explicit_agent_scope_writes_agent_instance_relation() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store
        .add_service("svc", agent_only_config("agent-a"))
        .await
        .unwrap();
    let instance_id = instance_id("svc", agent_scope("agent-a"));

    let relation = store
        .cache()
        .get_relation("agent_instances", "agent-a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        relation["instances"][0]["instance_id"],
        instance_id.to_string()
    );
    assert_eq!(relation["instances"][0]["service_name"], "svc");
    assert_eq!(
        relation["instances"][0]["scope"],
        serde_json::json!({
            "type": "agent",
            "agent_id": "agent-a"
        })
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn remove_service_clears_definition_and_all_instance_cache() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let mut config = stdio_config();
    config.mcpstore = Some(McpStoreExtension {
        scopes: ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([("agent-a".to_string(), ScopeDescriptor::default())]),
        },
        lifecycle: None,
        revision: 1,
        extra: Map::new(),
    });
    store.add_service("svc", config).await.unwrap();
    let store_id = store_instance_id("svc");
    let agent_id = instance_id("svc", agent_scope("agent-a"));

    store.remove_service("svc").await.unwrap();

    assert!(store
        .cache()
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_none());
    for instance_id in [store_id, agent_id] {
        assert!(store
            .cache()
            .get_entity("service_instances", &instance_id.to_string())
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_state("service_state", &instance_id.to_string())
            .await
            .unwrap()
            .is_none());
    }
    assert!(store
        .cache()
        .get_relation("agent_instances", "agent-a")
        .await
        .unwrap()
        .is_none());
    assert!(store.show_config().await.unwrap()["mcpServers"]
        .get("svc")
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn db_source_does_not_write_config_file_and_queues_add() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("test-db-source-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    store.add_service("svc", stdio_config()).await.unwrap();

    assert!(!std::path::Path::new(&path).exists());
    assert!(store
        .cache()
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_none());
    let events = store
        .cache()
        .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
        .await
        .unwrap();
    let event = events.values().next().unwrap();
    assert_eq!(event["type"], "ServiceAddRequested");
    assert_eq!(event["status"], "queued");
    assert_eq!(event["payload"]["service_name"], "svc");
}

#[tokio::test]
async fn db_source_rebuilds_definition_instance_tools_and_status_on_read() {
    let source_path = temp_config_path();
    let source = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(source_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-seed-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    source.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    let mut instance = source.find_instance(instance_id).await.unwrap();
    instance.tools = vec![crate::registry::ToolInfo {
        name: "echo".to_string(),
        title: None,
        description: "echo".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: None,
        annotations: None,
        meta: None,
    }];
    source.registry.register_instance(instance).await;
    source
        .cache_instance_connected(instance_id, &source.list_tools(instance_id).await.unwrap())
        .await
        .unwrap();
    source
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::StartRequested,
            1,
        )
        .await
        .unwrap();
    source
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::TransportConnected,
            2,
        )
        .await
        .unwrap();
    source
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::HealthObserved {
                health: crate::state::HealthState::Healthy,
                metrics: crate::state::HealthMetrics::default(),
                failure: None,
            },
            3,
        )
        .await
        .unwrap();
    source
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::ToolSyncSucceeded {
                tools: vec!["echo".to_string()],
            },
            4,
        )
        .await
        .unwrap();

    let db = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-read-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    copy_cache_snapshot(&source, &db).await;

    let instances = db.list_scope_instances(&store_scope()).await.unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].instance_id, instance_id);
    assert_eq!(db.list_tools(instance_id).await.unwrap()[0].name, "echo");
    let state = db.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(state.health, crate::state::HealthState::Healthy);
    let unchanged = db
        .record_instance_failure(instance_id, "must stay read-only".to_string())
        .await
        .unwrap();
    assert_eq!(unchanged, state);
    assert_eq!(
        db.state_manager.get(instance_id).await.unwrap().unwrap(),
        state
    );
    assert_eq!(
        db.show_config().await.unwrap()["mcpServers"]["svc"]["command"],
        "echo"
    );

    std::fs::remove_file(source_path).ok();
}

#[tokio::test]
async fn db_source_queues_config_scope_and_runtime_mutations_with_new_identity() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-queue-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let scope = agent_scope("agent-a");
    let instance_id = instance_id("svc", scope.clone());

    store.update_service("svc", stdio_config()).await.unwrap();
    store
        .patch_service("svc", serde_json::json!({"description": "patched"}))
        .await
        .unwrap();
    store
        .declare_service_scope("svc", &scope, ScopeDescriptor::default())
        .await
        .unwrap();
    store.remove_service_scope("svc", &scope).await.unwrap();
    store.reset_scope(&scope).await.unwrap();
    store.connect_service(instance_id).await.unwrap();
    store.disconnect_service(instance_id).await.unwrap();
    store.restart_service(instance_id).await.unwrap();
    store.remove_service("svc").await.unwrap();
    store.reset_config().await.unwrap();

    let events = store
        .cache()
        .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
        .await
        .unwrap();
    let by_type = events
        .values()
        .map(|event| (event["type"].as_str().unwrap(), event))
        .collect::<HashMap<_, _>>();
    for expected in [
        "ServiceUpdateRequested",
        "ServicePatchRequested",
        "ServiceScopeDeclareRequested",
        "ServiceScopeRemoveRequested",
        "ScopeResetRequested",
        "ServiceConnectRequested",
        "ServiceDisconnectRequested",
        "ServiceRestartRequested",
        "ServiceRemoveRequested",
        "StoreResetRequested",
    ] {
        assert!(by_type.contains_key(expected), "missing {expected}");
    }
    assert_eq!(
        by_type["ServiceScopeDeclareRequested"]["payload"]["scope"],
        serde_json::json!({"type": "agent", "agent_id": "agent-a"})
    );
    assert_eq!(
        by_type["ServiceConnectRequested"]["payload"]["instance_id"],
        instance_id.to_string()
    );
}

#[tokio::test]
async fn db_source_runtime_projection_methods_do_not_change_canonical_state() {
    let source_path = temp_config_path();
    let source = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(source_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-runtime-seed-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    source.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    let original_state = source
        .state_manager
        .get(instance_id)
        .await
        .unwrap()
        .unwrap();

    let db = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-runtime-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    copy_cache_snapshot(&source, &db).await;
    db.load_from_db().await.unwrap();
    db.cache_instance_connected(
        instance_id,
        &[crate::registry::ToolInfo {
            name: "echo".to_string(),
            title: None,
            description: "echo".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: None,
            meta: None,
        }],
    )
    .await
    .unwrap();

    let state = db.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(state.desired, original_state.desired);
    assert_eq!(state.phase, original_state.phase);
    assert_eq!(state.health, original_state.health);
    assert_eq!(state.recovery, original_state.recovery);
    assert_eq!(state.auth, original_state.auth);
    assert_eq!(state.tools, original_state.tools);
    assert_eq!(state.failure, original_state.failure);
    assert!(db
        .cache()
        .get_entity("tools", &format!("{instance_id}:echo"))
        .await
        .unwrap()
        .is_none());
    assert!(db
        .cache()
        .get_relation("instance_tools", &instance_id.to_string())
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(source_path).ok();
}

#[tokio::test]
async fn db_source_queues_tool_refresh_by_instance_without_writing_tools() {
    let source_path = temp_config_path();
    let source = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(source_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-tool-seed-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    source.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");

    let db = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("db-tool-queue-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    copy_cache_snapshot(&source, &db).await;

    let summary = db.list_changed_tools(instance_id, true).await.unwrap();

    assert!(!summary.changed);
    assert_eq!(summary.trigger, "queued_manual_force");
    assert_eq!(summary.details["queued"], true);
    assert_eq!(
        summary.details["queued_instances"],
        serde_json::json!([instance_id])
    );
    assert!(db
        .cache()
        .get_relation("instance_tools", &instance_id.to_string())
        .await
        .unwrap()
        .is_none());
    let events = db
        .cache()
        .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
        .await
        .unwrap();
    let event = events
        .values()
        .find(|event| event["type"] == "ServiceRefreshToolsRequested")
        .unwrap();
    assert_eq!(event["payload"]["instance_id"], instance_id.to_string());
    assert_eq!(event["payload"]["force_refresh"], true);

    std::fs::remove_file(source_path).ok();
}

#[tokio::test]
async fn openapi_import_persists_shared_analysis_result() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-import-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items": {
                "get": { "operationId": "listItems", "summary": "List items" },
                "post": { "operationId": "createItem", "requestBody": { "required": true } }
            },
            "/items/{sku}": {
                "get": {
                    "parameters": [{ "name": "sku", "in": "path", "required": true, "schema": { "type": "string" } }]
                }
            }
        }
    });

    let result = store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();
    let instance_id = store_instance_id("inventory");
    let pending = store.find_instance(instance_id).await.unwrap();
    assert_eq!(pending.applied_config_revision, None);
    assert!(pending.tools.is_empty());
    assert!(store
        .applied_openapi_configs
        .read()
        .await
        .get(&instance_id)
        .is_none());
    let pending_entity = store
        .cache()
        .get_entity("service_instances", &instance_id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert!(pending_entity["applied_config_revision"].is_null());
    let pending_state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(pending_state.phase, crate::state::RuntimePhase::Stopped);
    assert!(store
        .cache()
        .get_relation("instance_tools", &instance_id.to_string())
        .await
        .unwrap()
        .is_none());

    store.connect_service(instance_id).await.unwrap();

    assert_eq!(result.service_name, "inventory");
    assert_eq!(result.total_endpoints, 3);
    assert_eq!(result.component_types.tools, 1);
    assert_eq!(result.component_types.resources, 1);
    assert_eq!(result.component_types.resource_templates, 1);
    assert!(result.runtime_executable);

    let service = store.find_instance(instance_id).await.unwrap();
    assert_eq!(service.transport, "openapi");
    let connected_state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(connected_state.phase, crate::state::RuntimePhase::Running);
    assert_eq!(connected_state.health, crate::state::HealthState::Unknown);
    assert_eq!(
        connected_state.health_metrics,
        crate::state::HealthMetrics::default()
    );
    assert_eq!(
        service.applied_config_revision,
        Some(service.config_revision)
    );
    let applied_entity = store
        .cache()
        .get_entity("service_instances", &instance_id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        applied_entity["applied_config_revision"],
        serde_json::to_value(service.config_revision).unwrap()
    );

    let tools = store.list_tools(instance_id).await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "createItem");

    let call_result = store
        .call_tool(
            store_instance_id("inventory"),
            "createItem",
            serde_json::json!({"body": {"sku": "sku-1", "name": "apple"}}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["created"],
        serde_json::json!(true)
    );
    let observed_state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(observed_state.health, crate::state::HealthState::Healthy);
    assert_eq!(
        observed_state.health_metrics,
        crate::state::HealthMetrics::default()
    );

    let resources = store
        .list_resources(store_instance_id("inventory"))
        .await
        .unwrap();
    assert_eq!(resources[0].uri, "openapi://inventory/listItems");
    let resource = store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/listItems",
        )
        .await
        .unwrap();
    assert!(resource["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("apple"));

    let templates = store
        .list_resource_templates(store_instance_id("inventory"))
        .await
        .unwrap();
    assert_eq!(
        templates[0].uri_template,
        "openapi://inventory/get_items_sku/{sku}"
    );
    let templated = store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/get_items_sku/sku-1",
        )
        .await
        .unwrap();
    assert!(templated["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("sku-1"));
    assert!(store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/get_items_sku"
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("expected 1 path parameter"));
    assert!(store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/get_items_sku/sku-1/extra"
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("expected 1 path parameter"));

    let persisted = store
        .get_openapi_import("inventory")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.spec_info.title.as_deref(), Some("Inventory"));
    let last_import = store.last_openapi_import().await.unwrap().unwrap();
    assert_eq!(last_import.service_name, "inventory");
    assert_eq!(store.list_openapi_imports().await.unwrap().len(), 1);

    let inspect = store.cache_inspect().await.unwrap();
    assert_eq!(
        inspect["counts"]["states"]["openapi_import_context"],
        serde_json::json!(1)
    );
    assert_eq!(
        inspect["counts"]["states"]["openapi_imports"],
        serde_json::json!(1)
    );
    assert_eq!(
        inspect["counts"]["events"]["openapi_imports"],
        serde_json::json!(1)
    );
    assert_eq!(
        inspect["counts"]["entities"]["service_definitions"],
        serde_json::json!(1)
    );
    assert_eq!(inspect["counts"]["entities"]["tools"], serde_json::json!(1));
}

#[tokio::test]
async fn openapi_last_import_tracks_latest_successful_import() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-last-import-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    assert!(store.last_openapi_import().await.unwrap().is_none());

    let spec = |title: &str| {
        serde_json::json!({
            "openapi": "3.0.0",
            "info": {"title": title, "version": "1.0"},
            "paths": {
                "/ping": {
                    "get": {"operationId": "ping"}
                }
            }
        })
    };

    store
        .import_openapi_service_from_spec("alpha", "memory://alpha", spec("Alpha"))
        .await
        .unwrap();
    assert_eq!(
        store
            .last_openapi_import()
            .await
            .unwrap()
            .unwrap()
            .service_name,
        "alpha"
    );

    store
        .import_openapi_service_from_spec("beta", "memory://beta", spec("Beta"))
        .await
        .unwrap();
    assert_eq!(
        store
            .last_openapi_import()
            .await
            .unwrap()
            .unwrap()
            .service_name,
        "beta"
    );
}

#[tokio::test]
async fn removing_openapi_service_clears_import_state() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-remove-import-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": {"title": "Inventory", "version": "1.0"},
        "paths": {
            "/items": {
                "get": {"operationId": "listItems"}
            }
        }
    });

    store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();
    assert!(store
        .get_openapi_import("inventory")
        .await
        .unwrap()
        .is_some());
    assert!(store.last_openapi_import().await.unwrap().is_some());

    store.remove_service("inventory").await.unwrap();

    assert!(store
        .get_openapi_import("inventory")
        .await
        .unwrap()
        .is_none());
    assert!(store.last_openapi_import().await.unwrap().is_none());
}

#[tokio::test]
async fn openapi_import_rejects_existing_definition_without_mutating_sibling_scopes() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-duplicate-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let mut config = stdio_config();
    config.mcpstore = Some(McpStoreExtension {
        scopes: ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
            ]),
        },
        lifecycle: None,
        revision: 1,
        extra: Map::new(),
    });
    store.add_service("inventory", config).await.unwrap();

    let definition_before = store.registry.find_definition("inventory").await.unwrap();
    let instances_before = store
        .registry
        .list_instances()
        .await
        .into_iter()
        .map(|instance| (instance.instance_id, instance))
        .collect::<HashMap<_, _>>();
    let definition_cache_before = store
        .cache()
        .get_entity("service_definitions", "inventory")
        .await
        .unwrap()
        .unwrap();
    let mut instance_cache_before = HashMap::new();
    let mut connectivity_before = HashMap::new();
    for instance_id in instances_before.keys().copied() {
        instance_cache_before.insert(
            instance_id,
            store
                .cache()
                .get_entity("service_instances", &instance_id.to_string())
                .await
                .unwrap()
                .unwrap(),
        );
        connectivity_before.insert(instance_id, store.pool.is_connected(instance_id).await);
    }
    let config_before = store.show_config().await.unwrap();

    let error = store
        .import_openapi_service_from_spec(
            "inventory",
            "memory://inventory",
            serde_json::json!({
                "openapi": "3.0.0",
                "info": {"title": "Duplicate Inventory", "version": "1.0"},
                "paths": {
                    "/items": {
                        "get": {"operationId": "listItems"}
                    }
                }
            }),
        )
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("Service definition already exists: inventory"));
    assert_eq!(
        store.registry.find_definition("inventory").await.unwrap(),
        definition_before
    );
    assert_eq!(
        store
            .registry
            .list_instances()
            .await
            .into_iter()
            .map(|instance| (instance.instance_id, instance))
            .collect::<HashMap<_, _>>(),
        instances_before
    );
    assert_eq!(
        store
            .cache()
            .get_entity("service_definitions", "inventory")
            .await
            .unwrap()
            .unwrap(),
        definition_cache_before
    );
    for (instance_id, cached_before) in instance_cache_before {
        assert_eq!(
            store
                .cache()
                .get_entity("service_instances", &instance_id.to_string())
                .await
                .unwrap()
                .unwrap(),
            cached_before
        );
        assert_eq!(
            store.pool.is_connected(instance_id).await,
            connectivity_before[&instance_id]
        );
    }
    assert_eq!(store.show_config().await.unwrap(), config_before);
    assert!(store
        .get_openapi_import("inventory")
        .await
        .unwrap()
        .is_none());
    assert!(store.last_openapi_import().await.unwrap().is_none());
    assert!(store
        .cache()
        .get_all_events_async("openapi_imports")
        .await
        .unwrap()
        .is_empty());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn openapi_import_bundles_external_http_refs() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-external-ref-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let result = store
        .import_openapi_service("external", &format!("{base_url}/openapi.json"))
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("external"))
        .await
        .unwrap();

    assert_eq!(result.total_endpoints, 1);
    assert_eq!(result.component_types.resource_templates, 1);
    let component = &result.components[0];
    assert_eq!(component.name, "getItemByExternalRef");
    assert_eq!(
        component.input_schema["properties"]["id"],
        serde_json::json!({
            "type": "string",
            "description": "external item id",
            "x_mcpstore_parameter_in": "path"
        })
    );
    assert_eq!(
        component.input_schema["properties"]["verbose"]["type"],
        serde_json::json!("boolean")
    );

    let call_result = store
        .read_resource(
            store_instance_id("external"),
            "openapi://external/getItemByExternalRef/sku-1",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            call_result["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["id"],
        serde_json::json!("sku-1")
    );
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn openapi_import_bundles_external_yaml_http_refs() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-external-yaml-ref-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();

    let result = store
        .import_openapi_service(
            "external-yaml",
            &format!("{base_url}/openapi-yaml-ref.yaml"),
        )
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("external-yaml"))
        .await
        .unwrap();

    assert_eq!(result.total_endpoints, 1);
    assert_eq!(result.component_types.resource_templates, 1);
    let component = &result.components[0];
    assert_eq!(component.name, "getItemByExternalYamlRef");
    assert_eq!(
        component.input_schema["properties"]["id"],
        serde_json::json!({
            "type": "string",
            "description": "external YAML item id",
            "x_mcpstore_parameter_in": "path"
        })
    );

    let call_result = store
        .read_resource(
            store_instance_id("external-yaml"),
            "openapi://external-yaml/getItemByExternalYamlRef/sku-1",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            call_result["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["id"],
        serde_json::json!("sku-1")
    );
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn openapi_import_bundles_external_file_refs() {
    let base_url = spawn_openapi_http_fixture().await;
    let fixture_dir = std::env::temp_dir().join(format!(
        "mcpstore-openapi-file-ref-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(fixture_dir.join("components")).unwrap();
    let spec_path = fixture_dir.join("openapi.yaml");
    let components_path = fixture_dir.join("components").join("shared.yaml");
    std::fs::write(
        &components_path,
        r#"components:
  parameters:
    ItemId:
      name: id
      in: path
      required: true
      schema:
        $ref: '#/components/schemas/ItemId'
  schemas:
    ItemId:
      type: string
      description: local file item id
    Item:
      type: object
      properties:
        sku:
          $ref: '#/components/schemas/ItemId'
        name:
          type: string
"#,
    )
    .unwrap();
    std::fs::write(
        &spec_path,
        format!(
            r#"openapi: 3.0.0
info:
  title: Local File Refs
  version: '2026.1'
servers:
  - url: {base_url}
paths:
  /items/{{id}}:
    parameters:
      - $ref: components/shared.yaml#/components/parameters/ItemId
    get:
      operationId: getItemByLocalFileRef
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                $ref: components/shared.yaml#/components/schemas/Item
"#
        ),
    )
    .unwrap();

    let file_url = reqwest::Url::from_file_path(&spec_path)
        .unwrap()
        .to_string();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-file-ref-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let result = store
        .import_openapi_service("file-ref", &file_url)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("file-ref"))
        .await
        .unwrap();

    assert_eq!(result.total_endpoints, 1);
    assert_eq!(result.component_types.resource_templates, 1);
    let component = &result.components[0];
    assert_eq!(component.name, "getItemByLocalFileRef");
    assert_eq!(
        component.input_schema["properties"]["id"],
        serde_json::json!({
            "type": "string",
            "description": "local file item id",
            "x_mcpstore_parameter_in": "path"
        })
    );

    let call_result = store
        .read_resource(
            store_instance_id("file-ref"),
            "openapi://file-ref/getItemByLocalFileRef/sku-1",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            call_result["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["sku"],
        serde_json::json!("sku-1")
    );

    let path_store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-path-ref-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let path_result = path_store
        .import_openapi_service("path-ref", &spec_path.to_string_lossy())
        .await
        .unwrap();
    assert_eq!(path_result.components[0].name, "getItemByLocalFileRef");
    assert_eq!(
        path_result.components[0].input_schema["properties"]["id"]["description"],
        serde_json::json!("local file item id")
    );

    let _ = std::fs::remove_dir_all(fixture_dir);
}

#[tokio::test]
async fn openapi_bundle_caches_file_ref_documents_with_fingerprint() {
    let base_url = spawn_openapi_http_fixture().await;
    let fixture_dir = std::env::temp_dir().join(format!(
        "mcpstore-openapi-file-cache-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(fixture_dir.join("components")).unwrap();
    let spec_path = fixture_dir.join("openapi.yaml");
    let components_path = fixture_dir.join("components").join("shared.yaml");
    std::fs::write(
        &components_path,
        r#"components:
  schemas:
    ItemId:
      type: string
      description: local file item id
    Item:
      type: object
      properties:
        sku:
          $ref: '#/components/schemas/ItemId'
"#,
    )
    .unwrap();
    std::fs::write(
        &spec_path,
        format!(
            r#"openapi: 3.0.0
info:
  title: Local File Cache Refs
  version: '2026.1'
servers:
  - url: {base_url}
paths:
  /items/{{id}}:
    get:
      operationId: getItemByCachedLocalFileRef
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                $ref: components/shared.yaml#/components/schemas/Item
"#
        ),
    )
    .unwrap();

    let file_url = reqwest::Url::from_file_path(&spec_path)
        .unwrap()
        .to_string();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-file-ref-document-cache-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();

    let first_artifact = store.bundle_openapi_artifact(&file_url).await.unwrap();
    let first_description = &first_artifact.bundle["paths"]["/items/{id}"]["get"]["responses"]
        ["200"]["content"]["application/json"]["schema"]["properties"]["sku"]["description"];
    assert_eq!(first_description, &serde_json::json!("local file item id"));

    let states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    let cached = states.values().next().unwrap();
    assert_eq!(cached["cache_version"], serde_json::json!(1));
    assert_eq!(cached["source"], serde_json::json!("file"));
    assert!(cached["url"].as_str().unwrap().starts_with("file://"));
    assert!(cached["content_hash"]
        .as_str()
        .unwrap()
        .starts_with("blake3:"));
    assert!(cached["content_length"].as_u64().unwrap() > 0);
    assert!(cached["file_size"].as_u64().unwrap() > 0);
    assert!(cached["file_modified_unix_millis"].as_i64().unwrap() > 0);
    let first_hash = cached["content_hash"].as_str().unwrap().to_string();

    std::fs::write(
        &components_path,
        r#"components:
  schemas:
    ItemId:
      type: string
      description: updated local file item id after cache invalidation
    Item:
      type: object
      properties:
        sku:
          $ref: '#/components/schemas/ItemId'
"#,
    )
    .unwrap();

    let second_artifact = store.bundle_openapi_artifact(&file_url).await.unwrap();
    let second_description = &second_artifact.bundle["paths"]["/items/{id}"]["get"]["responses"]
        ["200"]["content"]["application/json"]["schema"]["properties"]["sku"]["description"];
    assert_eq!(
        second_description,
        &serde_json::json!("updated local file item id after cache invalidation")
    );
    let updated_states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    let updated_cached = updated_states.values().next().unwrap();
    assert_ne!(
        updated_cached["content_hash"],
        serde_json::json!(first_hash)
    );

    let _ = std::fs::remove_dir_all(fixture_dir);
}

#[tokio::test]
async fn openapi_bundle_spec_returns_external_refs_without_importing() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-bundle-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let bundled = store
        .bundle_openapi_spec(&format!("{base_url}/openapi.json"))
        .await
        .unwrap();

    assert_eq!(bundled["info"]["title"], serde_json::json!("External Refs"));
    assert_eq!(
        bundled["paths"]["/items/{id}"]["parameters"][0]["schema"],
        serde_json::json!({
            "type": "string",
            "description": "external item id"
        })
    );
    assert_eq!(
        bundled["paths"]["/items/{id}"]["get"]["responses"]["200"]["content"]["application/json"]
            ["schema"]["properties"]["id"],
        serde_json::json!({
            "type": "string",
            "description": "external item id"
        })
    );
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(store.list_openapi_imports().await.unwrap().is_empty());
}

#[tokio::test]
async fn openapi_bundle_artifact_reports_dependencies_without_importing() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-bundle-artifact-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let root_url = format!("{base_url}/openapi.json");
    let artifact = store.bundle_openapi_artifact(&root_url).await.unwrap();

    assert_eq!(artifact.spec_url, root_url);
    assert_eq!(
        artifact.bundle["info"]["title"],
        serde_json::json!("External Refs")
    );
    assert!(artifact.diagnostics.is_empty());
    assert!(artifact
        .documents
        .iter()
        .any(|document| document.url == root_url && document.role == "root"));
    assert!(artifact.documents.iter().any(|document| {
        document.url == format!("{base_url}/components.json") && document.role == "external"
    }));
    for document in &artifact.documents {
        assert!(document.content_hash.starts_with("blake3:"));
        assert!(document.content_length > 0);
    }
    assert!(artifact.dependencies.iter().any(|dependency| {
        dependency.source_document == root_url
            && dependency.source_ref == "components.json#/components/parameters/ItemId"
            && dependency.target_document == format!("{base_url}/components.json")
            && dependency.pointer.as_deref() == Some("/components/parameters/ItemId")
    }));
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(store.list_openapi_imports().await.unwrap().is_empty());
}

#[tokio::test]
async fn openapi_bundle_writes_external_ref_document_cache() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-ref-document-cache-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();

    let root_url = format!("{base_url}/openapi.json");
    let artifact = store.bundle_openapi_artifact(&root_url).await.unwrap();

    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(artifact.documents.iter().any(|document| {
        document.url == format!("{base_url}/components.json") && document.role == "external"
    }));
    let states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    let cached = states.values().next().unwrap();
    assert_eq!(cached["cache_version"], serde_json::json!(1));
    assert_eq!(cached["url"], format!("{base_url}/components.json"));
    assert_eq!(cached["source"], serde_json::json!("http"));
    assert!(cached["content_hash"]
        .as_str()
        .unwrap()
        .starts_with("blake3:"));
    assert!(cached["content_length"].as_u64().unwrap() > 0);
    assert!(cached["expires_at"].as_i64().unwrap() > cached["fetched_at"].as_i64().unwrap());
    assert_eq!(
        cached["document"]["components"]["schemas"]["ItemId"]["description"],
        serde_json::json!("external item id")
    );

    let second_artifact = store.bundle_openapi_artifact(&root_url).await.unwrap();
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(second_artifact.documents.iter().any(|document| {
        document.url == format!("{base_url}/components.json") && document.role == "external"
    }));
    assert!(store.list_openapi_imports().await.unwrap().is_empty());
}

#[tokio::test]
async fn openapi_bundle_ref_cache_policy_sets_ttl() {
    let (base_url, _components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-ref-cache-policy-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let root_url = format!("{base_url}/openapi.json");
    store
        .bundle_openapi_artifact_with_options(
            &root_url,
            crate::openapi::OpenApiBundleOptions {
                ref_cache: crate::openapi::OpenApiRefCachePolicy {
                    enabled: true,
                    ttl_seconds: 42,
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    let cached = states.values().next().unwrap();
    assert_eq!(cached["ttl_seconds"], serde_json::json!(42));
    assert_eq!(
        cached["expires_at"].as_i64().unwrap() - cached["fetched_at"].as_i64().unwrap(),
        42
    );
}

#[tokio::test]
async fn openapi_bundle_ref_cache_policy_can_disable_shared_cache() {
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-ref-cache-disabled-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();
    let options = crate::openapi::OpenApiBundleOptions {
        ref_cache: crate::openapi::OpenApiRefCachePolicy {
            enabled: false,
            ttl_seconds: 300,
        },
        ..Default::default()
    };

    let root_url = format!("{base_url}/openapi.json");
    store
        .bundle_openapi_artifact_with_options(&root_url, options.clone())
        .await
        .unwrap();
    store
        .bundle_openapi_artifact_with_options(&root_url, options)
        .await
        .unwrap();

    assert_eq!(components_requests.load(Ordering::SeqCst), 2);
    let states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert!(states.is_empty());
}

#[tokio::test]
async fn openapi_bundle_revalidates_expired_http_ref_cache_with_etag() {
    let (base_url, components_requests, conditional_requests) =
        spawn_openapi_conditional_ref_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-ref-document-revalidate-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();
    let root_url = format!("{base_url}/openapi.json");

    let first_artifact = store.bundle_openapi_artifact(&root_url).await.unwrap();
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert_eq!(conditional_requests.load(Ordering::SeqCst), 0);
    assert_eq!(
        first_artifact.bundle["paths"]["/items/{id}"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"]["id"]["description"],
        serde_json::json!("conditional external item id")
    );

    let states = store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    let (cache_key, cached) = states.into_iter().next().unwrap();
    assert_eq!(cached["etag"], serde_json::json!("\"components-v1\""));
    assert_eq!(
        cached["last_modified"],
        serde_json::json!("Tue, 01 Jan 2030 00:00:00 GMT")
    );
    let original_expires_at = cached["expires_at"].as_i64().unwrap();
    let mut expired = cached.as_object().unwrap().clone();
    expired.insert(
        "expires_at".to_string(),
        serde_json::json!(chrono::Utc::now().timestamp() - 1),
    );
    store
        .cache()
        .put_state(
            "openapi_ref_documents",
            &cache_key,
            serde_json::Value::Object(expired),
        )
        .await
        .unwrap();

    let second_artifact = store.bundle_openapi_artifact(&root_url).await.unwrap();
    assert_eq!(components_requests.load(Ordering::SeqCst), 2);
    assert_eq!(conditional_requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        second_artifact.bundle["paths"]["/items/{id}"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"]["id"]["description"],
        serde_json::json!("conditional external item id")
    );
    let refreshed = store
        .cache()
        .get_state("openapi_ref_documents", &cache_key)
        .await
        .unwrap()
        .unwrap();
    assert!(refreshed["expires_at"].as_i64().unwrap() >= original_expires_at);
    assert_eq!(refreshed["etag"], serde_json::json!("\"components-v1\""));
}

#[tokio::test]
async fn redis_backend_reuses_openapi_ref_document_cache_between_store_instances_when_available() {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        return;
    };
    let (base_url, components_requests) = spawn_openapi_spec_ref_fixture().await;
    let namespace = format!("openapi-ref-document-cache-{}", uuid::Uuid::new_v4());
    let first = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url.clone()),
        namespace: Some(namespace.clone()),
    })
    .unwrap();
    let second = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url),
        namespace: Some(namespace),
    })
    .unwrap();
    let root_url = format!("{base_url}/openapi.json");

    let first_artifact = first.bundle_openapi_artifact(&root_url).await.unwrap();
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(first_artifact.documents.iter().any(|document| {
        document.url == format!("{base_url}/components.json") && document.role == "external"
    }));

    let second_artifact = second.bundle_openapi_artifact(&root_url).await.unwrap();
    assert_eq!(components_requests.load(Ordering::SeqCst), 1);
    assert!(second_artifact.documents.iter().any(|document| {
        document.url == format!("{base_url}/components.json") && document.role == "external"
    }));
}

#[tokio::test]
async fn openapi_import_parses_yaml_from_url() {
    let base_url = spawn_openapi_yaml_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-yaml-url-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let result = store
        .import_openapi_service("yaml", &format!("{base_url}/openapi.yaml"))
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("yaml"))
        .await
        .unwrap();

    assert_eq!(result.spec_info.title.as_deref(), Some("YAML Inventory"));
    assert_eq!(result.component_types.resources, 1);
    let call_result = store
        .read_resource(store_instance_id("yaml"), "openapi://yaml/listYamlItems")
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            call_result["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["items"][0],
        serde_json::json!("yaml")
    );
}

#[tokio::test]
async fn openapi_import_parses_yaml_spec_text() {
    let base_url = spawn_openapi_yaml_fixture().await;
    let spec_text = format!(
        r#"openapi: 3.0.0
info:
  title: YAML Text Inventory
  version: '2026.1'
servers:
  - url: {base_url}
paths:
  /items:
    get:
      operationId: listYamlTextItems
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                type: object
"#
    );
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-yaml-text-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let result = store
        .import_openapi_service_from_spec_text("yaml-text", "memory://yaml-text", &spec_text)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("yaml-text"))
        .await
        .unwrap();

    assert_eq!(
        result.spec_info.title.as_deref(),
        Some("YAML Text Inventory")
    );
    assert_eq!(result.component_types.resources, 1);
    let call_result = store
        .read_resource(
            store_instance_id("yaml-text"),
            "openapi://yaml-text/listYamlTextItems",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            call_result["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["items"][0],
        serde_json::json!("yaml")
    );
}

#[tokio::test]
async fn openapi_tool_http_error_returns_tool_error_without_marking_service_failed() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-http-error-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items/reject": {
                "post": { "operationId": "rejectItem", "requestBody": { "required": true } }
            }
        }
    });

    store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("inventory"))
        .await
        .unwrap();

    let instance_id = store_instance_id("inventory");
    let execution = store
        .start_tool_execution(
            instance_id,
            "rejectItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
            crate::transport::McpExecutionOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(execution.instance_id(), instance_id);
    assert!(!execution.supports_cancellation());
    assert!(execution.request_id().is_none());
    let crate::transport::McpToolExecution::Immediate {
        result: call_result,
    } = execution.wait().await.unwrap()
    else {
        panic!("OpenAPI execution must complete immediately");
    };
    assert!(call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    let payload = serde_json::from_str::<serde_json::Value>(text).unwrap();
    assert_eq!(payload["status"], serde_json::json!(404));
    assert!(payload["body"]["error"]
        .as_str()
        .unwrap()
        .contains("POST /items/reject"));
    let completed = store
        .event_history(10)
        .await
        .into_iter()
        .find(|event| event.event_type == "TOOL_CALL_COMPLETED")
        .expect("OpenAPI execution must publish the existing completion event");
    assert_eq!(
        completed.payload["instance_id"],
        serde_json::json!(instance_id)
    );
    assert_eq!(completed.payload["status"], serde_json::json!("error"));

    let service = store
        .find_instance(store_instance_id("inventory"))
        .await
        .unwrap();
    let state = store
        .state_manager
        .get(service.instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.phase, crate::state::RuntimePhase::Running);
    assert_eq!(state.failure, None);
}

#[tokio::test]
async fn openapi_runtime_honors_import_timeout() {
    let base_url = spawn_openapi_slow_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-timeout-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Slow", "version": "1.0" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/slow": {
                "post": { "operationId": "createSlowItem", "requestBody": { "required": true } }
            }
        }
    });

    store
        .import_openapi_service_from_spec_with_options(
            "slow",
            "memory://slow",
            spec,
            crate::openapi::OpenApiImportOptions {
                timeout_millis: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("slow"))
        .await
        .unwrap();

    let started_at = Instant::now();
    let err = store
        .call_tool(
            store_instance_id("slow"),
            "createSlowItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("OpenAPI request failed"), "{err}");
    assert!(started_at.elapsed() < Duration::from_millis(200), "{err}");
}

#[tokio::test]
async fn openapi_import_honors_fetch_timeout() {
    let base_url = spawn_openapi_slow_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-fetch-timeout-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let started_at = Instant::now();
    let err = store
        .import_openapi_service_with_options(
            "slow-import",
            &format!("{base_url}/openapi.json"),
            crate::openapi::OpenApiImportOptions {
                fetch_timeout_millis: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("OpenAPI spec fetch failed"), "{err}");
    assert!(started_at.elapsed() < Duration::from_millis(200), "{err}");
}

#[tokio::test]
async fn openapi_bundle_honors_fetch_timeout() {
    let base_url = spawn_openapi_slow_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-bundle-timeout-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();

    let started_at = Instant::now();
    let err = store
        .bundle_openapi_artifact_with_options(
            &format!("{base_url}/openapi.json"),
            crate::openapi::OpenApiBundleOptions {
                timeout_millis: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("OpenAPI spec fetch failed"), "{err}");
    assert!(started_at.elapsed() < Duration::from_millis(200), "{err}");
}

#[tokio::test]
async fn openapi_resources_preserve_response_mime_type() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-response-mime-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/plain": {
                "get": {
                    "operationId": "getPlainInventory",
                    "responses": {
                        "200": {
                            "description": "Plain response",
                            "content": { "text/plain": { "schema": { "type": "string" } } }
                        }
                    }
                }
            },
            "/items": {
                "get": {
                    "operationId": "listItems",
                    "responses": {
                        "200": {
                            "description": "JSON response",
                            "content": { "application/json": { "schema": { "type": "object" } } }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("inventory"))
        .await
        .unwrap();

    let resources = store
        .list_resources(store_instance_id("inventory"))
        .await
        .unwrap();
    let plain = resources
        .iter()
        .find(|resource| resource.name == "getPlainInventory")
        .unwrap();
    assert_eq!(plain.mime_type.as_deref(), Some("text/plain"));

    let resource = store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/getPlainInventory",
        )
        .await
        .unwrap();
    assert_eq!(
        resource["contents"][0]["mimeType"],
        serde_json::json!("text/plain")
    );
    assert_eq!(
        resource["contents"][0]["text"],
        serde_json::json!("plain inventory")
    );

    let json_resource = store
        .read_resource(
            store_instance_id("inventory"),
            "openapi://inventory/listItems",
        )
        .await
        .unwrap();
    assert_eq!(
        json_resource["contents"][0]["mimeType"],
        serde_json::json!("application/json")
    );
}

#[tokio::test]
async fn openapi_json_responses_filter_write_only_fields() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-response-write-only-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();
    let profile_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "readOnly": true },
            "username": { "type": "string" },
            "password": { "type": "string", "writeOnly": true },
            "tokens": { "type": "array", "items": { "type": "string", "writeOnly": true } },
            "metadata": {
                "type": "object",
                "properties": {
                    "public": { "type": "string" },
                    "secret": { "type": "string", "writeOnly": true }
                }
            }
        }
    });
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Profiles", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/profile": {
                "get": {
                    "operationId": "getProfile",
                    "responses": {
                        "200": {
                            "description": "Profile response",
                            "content": { "application/json": { "schema": profile_schema.clone() } }
                        }
                    }
                },
                "post": {
                    "operationId": "createProfile",
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
                    "responses": {
                        "200": {
                            "description": "Profile response",
                            "content": { "application/json": { "schema": profile_schema } }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("profiles", "memory://profiles", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("profiles"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("profiles"),
            "createProfile",
            serde_json::json!({ "body": { "username": "alice" } }),
        )
        .await
        .unwrap();
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    let body = serde_json::from_str::<serde_json::Value>(text).unwrap();
    assert_eq!(body["id"], serde_json::json!("profile-1"));
    assert_eq!(body["username"], serde_json::json!("alice"));
    assert!(body.get("password").is_none());
    assert_eq!(body["tokens"], serde_json::json!([]));
    assert_eq!(body["metadata"], serde_json::json!({ "public": "visible" }));

    let resource = store
        .read_resource(
            store_instance_id("profiles"),
            "openapi://profiles/getProfile",
        )
        .await
        .unwrap();
    let body = serde_json::from_str::<serde_json::Value>(
        resource["contents"][0]["text"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(body["id"], serde_json::json!("profile-1"));
    assert_eq!(body["username"], serde_json::json!("alice"));
    assert!(body.get("password").is_none());
    assert_eq!(body["tokens"], serde_json::json!([]));
    assert_eq!(body["metadata"], serde_json::json!({ "public": "visible" }));
}

#[tokio::test]
async fn openapi_json_responses_validate_declared_schema() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-response-schema-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let profile_schema = serde_json::json!({
        "type": "object",
        "required": ["id", "username", "metadata"],
        "properties": {
            "id": { "type": "string" },
            "username": { "type": "string", "minLength": 1 },
            "metadata": {
                "type": "object",
                "required": ["public"],
                "properties": { "public": { "type": "string" } },
                "additionalProperties": false
            },
            "tags": { "type": "array", "minItems": 1, "items": { "type": "string" } }
        }
    });
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Invalid Profiles", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/invalid-profile": {
                "get": {
                    "operationId": "getInvalidProfile",
                    "responses": {
                        "200": {
                            "description": "Invalid profile response",
                            "content": { "application/json": { "schema": profile_schema.clone() } }
                        }
                    }
                },
                "post": {
                    "operationId": "createInvalidProfile",
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
                    "responses": {
                        "200": {
                            "description": "Invalid profile response",
                            "content": { "application/json": { "schema": profile_schema } }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("invalid-profiles", "memory://invalid-profiles", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("invalid-profiles"))
        .await
        .unwrap();

    let tool_error = store
        .call_tool(
            store_instance_id("invalid-profiles"),
            "createInvalidProfile",
            serde_json::json!({ "body": {} }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(tool_error.contains("Invalid OpenAPI response for createInvalidProfile"));
    assert!(tool_error.contains("response.id must be a string"));
    assert!(tool_error.contains("response.username is required"));
    assert!(tool_error.contains("response.metadata.public is required"));
    assert!(tool_error.contains("response.tags must contain at least 1 item(s)"));

    let resource_error = store
        .read_resource(
            store_instance_id("invalid-profiles"),
            "openapi://invalid-profiles/getInvalidProfile",
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(resource_error.contains("Invalid OpenAPI response for getInvalidProfile"));
    assert!(resource_error.contains("response.id must be a string"));
}

#[tokio::test]
async fn openapi_runtime_sends_accept_for_supported_response_media_types() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-response-accept-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Negotiated", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/negotiated": {
                "get": {
                    "operationId": "getNegotiatedInventory",
                    "responses": {
                        "200": {
                            "description": "Negotiated response",
                            "content": {
                                "image/png": { "schema": { "type": "string", "format": "binary" } },
                                "application/json": { "schema": { "type": "object" } },
                                "text/plain": { "schema": { "type": "string" } }
                            }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("negotiated", "memory://negotiated", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("negotiated"))
        .await
        .unwrap();

    let resources = store
        .list_resources(store_instance_id("negotiated"))
        .await
        .unwrap();
    assert_eq!(resources[0].mime_type.as_deref(), Some("application/json"));

    let resource = store
        .read_resource(
            store_instance_id("negotiated"),
            "openapi://negotiated/getNegotiatedInventory",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            resource["contents"][0]["text"].as_str().unwrap()
        )
        .unwrap()["received"],
        serde_json::json!("accept")
    );
}

#[tokio::test]
async fn openapi_tool_returns_image_content_for_binary_image_response() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-image-response-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Images", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/image": {
                "post": {
                    "operationId": "renderImage",
                    "responses": {
                        "200": {
                            "description": "PNG response",
                            "content": { "image/png": { "schema": { "type": "string", "format": "binary" } } }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("images", "memory://images", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("images"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("images"),
            "renderImage",
            serde_json::json!({}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Image {
        data, mime_type, ..
    } = &call_result.content[0]
    else {
        panic!("expected image content");
    };
    assert_eq!(mime_type, "image/png");
    assert_eq!(
        data,
        &base64::engine::general_purpose::STANDARD.encode("png-bytes")
    );
}

#[tokio::test]
async fn openapi_resource_returns_blob_for_binary_response() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-blob-response-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Documents", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/document": {
                "get": {
                    "operationId": "getDocument",
                    "responses": {
                        "200": {
                            "description": "PDF response",
                            "content": { "application/pdf": { "schema": { "type": "string", "format": "binary" } } }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("documents", "memory://documents", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("documents"))
        .await
        .unwrap();

    let resources = store
        .list_resources(store_instance_id("documents"))
        .await
        .unwrap();
    assert_eq!(resources[0].mime_type.as_deref(), Some("application/pdf"));

    let resource = store
        .read_resource(
            store_instance_id("documents"),
            "openapi://documents/getDocument",
        )
        .await
        .unwrap();
    assert_eq!(
        resource["contents"][0]["mimeType"],
        serde_json::json!("application/pdf")
    );
    assert_eq!(
        resource["contents"][0]["blob"],
        serde_json::json!(base64::engine::general_purpose::STANDARD.encode("%PDF fixture"))
    );
    assert!(resource["contents"][0].get("text").is_none());
}

#[tokio::test]
async fn openapi_tools_support_common_request_body_media_types() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-body-media-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Forms", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/forms/urlencoded": {
                "post": {
                    "operationId": "submitUrlencoded",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/x-www-form-urlencoded": {
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
                            }
                        }
                    }
                }
            },
            "/forms/multipart": {
                "post": {
                    "operationId": "submitMultipart",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
                            }
                        }
                    }
                }
            },
            "/forms/text": {
                "post": {
                    "operationId": "submitText",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "text/plain": { "schema": { "type": "string" } }
                        }
                    }
                }
            },
            "/forms/file": {
                "post": {
                    "operationId": "submitFile",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": { "file": { "type": "string", "format": "binary" } }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/files": {
                "post": {
                    "operationId": "submitFiles",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": {
                                            "type": "array",
                                            "items": { "type": "string", "format": "binary" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("forms", "memory://forms", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("forms"))
        .await
        .unwrap();

    let tools = store.list_tools(store_instance_id("forms")).await.unwrap();
    let submit_files = tools
        .iter()
        .find(|tool| tool.name == "submitFiles")
        .unwrap();
    assert_eq!(
        submit_files.input_schema["properties"]["body"]["properties"]["files"]["type"],
        serde_json::json!("array")
    );
    assert_eq!(
        submit_files.input_schema["properties"]["body"]["properties"]["files"]["items"]
            ["x_mcpstore_file"],
        serde_json::json!(true)
    );

    for (tool_name, body, expected) in [
        (
            "submitUrlencoded",
            serde_json::json!({"name": "apple"}),
            "urlencoded",
        ),
        (
            "submitMultipart",
            serde_json::json!({"name": "apple"}),
            "multipart",
        ),
        ("submitText", serde_json::json!("plain-body"), "text"),
    ] {
        let call_result = store
            .call_tool(
                store_instance_id("forms"),
                tool_name,
                serde_json::json!({"body": body}),
            )
            .await
            .unwrap();
        assert!(!call_result.is_error);
        let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
            serde_json::json!(expected)
        );
    }

    let file_path = write_temp_file("apple-bytes");
    for file in [
        serde_json::json!({
            "bytes": base64::engine::general_purpose::STANDARD.encode("apple-bytes"),
            "filename": "apple.txt",
            "mimeType": "text/plain"
        }),
        serde_json::json!({
            "path": file_path.clone(),
            "filename": "apple.txt",
            "mimeType": "text/plain"
        }),
    ] {
        let call_result = store
            .call_tool(
                store_instance_id("forms"),
                "submitFile",
                serde_json::json!({"body": {"file": file}}),
            )
            .await
            .unwrap();
        assert!(!call_result.is_error);
        let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
            serde_json::json!("file")
        );
    }
    std::fs::remove_file(file_path).ok();

    let call_result = store
        .call_tool(
            store_instance_id("forms"),
            "submitFiles",
            serde_json::json!({"body": {"files": [
                {
                    "bytes": base64::engine::general_purpose::STANDARD.encode("apple-bytes"),
                    "filename": "apple.txt",
                    "mimeType": "text/plain"
                },
                {
                    "bytes": base64::engine::general_purpose::STANDARD.encode("pear-bytes"),
                    "filename": "pear.txt",
                    "mimeType": "text/plain"
                }
            ]}}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
        serde_json::json!("files")
    );

    let invalid_files = store
        .call_tool(
            store_instance_id("forms"),
            "submitFiles",
            serde_json::json!({"body": {"files": ["not-a-file"]}}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(invalid_files.contains("body.files[0]"));
}

#[tokio::test]
async fn openapi_tools_serialize_parameters_by_openapi_style() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-parameter-style-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Search", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/search/{ids}": {
                "post": {
                    "operationId": "searchItems",
                    "parameters": [
                        { "name": "ids", "in": "path", "required": true, "schema": { "type": "array", "items": { "type": "string" } } },
                        { "name": "tag", "in": "query", "style": "form", "explode": true, "schema": { "type": "array", "items": { "type": "string" } } },
                        { "name": "compact", "in": "query", "style": "form", "explode": false, "schema": { "type": "array", "items": { "type": "string" } } },
                        { "name": "x-flags", "in": "header", "schema": { "type": "array", "items": { "type": "string" } } }
                    ]
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("search", "memory://search", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("search"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("search"),
            "searchItems",
            serde_json::json!({
                "ids": ["a", "b"],
                "tag": ["red", "blue"],
                "compact": ["one", "two"],
                "x-flags": ["fast", "safe"]
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
        serde_json::json!("parameters")
    );
}

#[tokio::test]
async fn openapi_query_parameters_honor_allow_reserved() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-allow-reserved-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Reserved", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/reserved": {
                "post": {
                    "operationId": "sendReservedQuery",
                    "parameters": [
                        { "name": "raw", "in": "query", "allowReserved": true, "schema": { "type": "string" } },
                        { "name": "encoded", "in": "query", "schema": { "type": "string" } }
                    ]
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("reserved", "memory://reserved", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("reserved"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("reserved"),
            "sendReservedQuery",
            serde_json::json!({
                "raw": "https://example.com/a,b;c=1",
                "encoded": "https://example.com/a,b;c=1"
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
        serde_json::json!("reserved")
    );
}

#[tokio::test]
async fn openapi_query_parameters_support_deep_object_style() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-deep-object-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Deep Object", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/deep": {
                "post": {
                    "operationId": "sendDeepObjectQuery",
                    "parameters": [
                        { "name": "filter", "in": "query", "style": "deepObject", "explode": true, "schema": { "type": "object", "properties": { "color": { "type": "string" }, "size": { "type": "string" } } } }
                    ]
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("deep", "memory://deep", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("deep"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("deep"),
            "sendDeepObjectQuery",
            serde_json::json!({
                "filter": {
                    "color": "red",
                    "size": "large"
                }
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
        serde_json::json!("deep-object")
    );
}

#[tokio::test]
async fn openapi_path_parameters_support_label_and_matrix_styles() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-path-style-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Path Styles", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/styled/{label}/{id}": {
                "post": {
                    "operationId": "sendStyledPath",
                    "parameters": [
                        { "name": "label", "in": "path", "required": true, "style": "label", "schema": { "type": "array", "items": { "type": "string" } } },
                        { "name": "id", "in": "path", "required": true, "style": "matrix", "explode": true, "schema": { "type": "array", "items": { "type": "string" } } }
                    ]
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("pathstyles", "memory://pathstyles", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("pathstyles"))
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            store_instance_id("pathstyles"),
            "sendStyledPath",
            serde_json::json!({
                "label": ["red", "blue"],
                "id": ["sku-1", "sku-2"]
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
        serde_json::json!("styled-path")
    );
}

#[tokio::test]
async fn openapi_tools_reject_missing_required_arguments_before_request() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-required-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Required", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items/{sku}": {
                "post": {
                    "operationId": "updateItem",
                    "parameters": [{ "name": "sku", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object" } } } }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("required", "memory://required", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("required"))
        .await
        .unwrap();

    let missing_path = store
        .call_tool(
            store_instance_id("required"),
            "updateItem",
            serde_json::json!({"body": {"name": "apple"}}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(missing_path.contains("path.sku"));

    let missing_body = store
        .call_tool(
            store_instance_id("required"),
            "updateItem",
            serde_json::json!({"sku": "sku-1"}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(missing_body.contains("body"));
}

#[tokio::test]
async fn openapi_tools_honor_read_only_and_write_only_request_fields() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-read-write-only-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Read Write Only", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items": {
                "post": {
                    "operationId": "createReadWriteItem",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["id", "name", "secret"],
                                    "properties": {
                                        "id": { "type": "string", "readOnly": true },
                                        "name": { "type": "string" },
                                        "secret": { "type": "string", "writeOnly": true }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("readonly", "memory://readonly", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("readonly"))
        .await
        .unwrap();

    let tools = store
        .list_tools(store_instance_id("readonly"))
        .await
        .unwrap();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "createReadWriteItem")
        .unwrap();
    assert!(tool.input_schema["properties"]["body"]["properties"]
        .get("id")
        .is_none());
    assert_eq!(
        tool.input_schema["properties"]["body"]["properties"]["secret"]["writeOnly"],
        serde_json::json!(true)
    );
    assert_eq!(
        tool.input_schema["properties"]["body"]["required"],
        serde_json::json!(["name", "secret"])
    );

    let read_only_error = store
        .call_tool(
            store_instance_id("readonly"),
            "createReadWriteItem",
            serde_json::json!({
                "body": { "id": "server-generated", "name": "apple", "secret": "token" }
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(read_only_error.contains("body.id is readOnly and cannot be sent in a request"));

    let call_result = store
        .call_tool(
            store_instance_id("readonly"),
            "createReadWriteItem",
            serde_json::json!({
                "body": { "name": "apple", "secret": "token" }
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
}

#[tokio::test]
async fn openapi_tools_validate_input_schema_before_request() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!(
            "openapi-schema-validation-{}",
            uuid::Uuid::new_v4()
        )),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Validation", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items": {
                "post": {
                    "operationId": "createValidatedItem",
                    "parameters": [
                        { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 20 } },
                        { "name": "status", "in": "query", "schema": { "type": "string", "enum": ["draft", "published"] } },
                        { "name": "request_id", "in": "query", "schema": { "type": "string", "format": "uuid" } },
                        { "name": "filter_pattern", "in": "query", "schema": { "type": "string", "format": "regex" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["name"],
                                    "properties": {
                                        "name": { "type": "string", "minLength": 3, "pattern": "^item-[0-9]+$" },
                                        "code": { "type": "string", "maxLength": 4 },
                                        "price": { "type": "number", "minimum": 0, "exclusiveMinimum": true },
                                        "discount": { "type": "number", "exclusiveMaximum": 1 },
                                        "quantity": { "type": "integer", "multipleOf": 5 },
                                        "publish_date": { "type": "string", "format": "date" },
                                        "updated_at": { "type": "string", "format": "date-time" },
                                        "callback": { "type": "string", "format": "url" },
                                        "resource_uri": { "type": "string", "format": "uri" },
                                        "resource_ref": { "type": "string", "format": "uri-reference" },
                                        "unicode_iri": { "type": "string", "format": "iri" },
                                        "unicode_ref": { "type": "string", "format": "iri-reference" },
                                        "contact": { "type": "string", "format": "email" },
                                        "host": { "type": "string", "format": "hostname" },
                                        "ipv4": { "type": "string", "format": "ipv4" },
                                        "ipv6": { "type": "string", "format": "ipv6" },
                                        "pointer": { "type": "string", "format": "json-pointer" },
                                        "relative_pointer": { "type": "string", "format": "relative-json-pointer" },
                                        "composed": { "allOf": [{ "type": "string", "minLength": 3 }, { "type": "string", "pattern": "^ok-" }] },
                                        "choice": { "anyOf": [{ "type": "string", "enum": ["alpha"] }, { "type": "integer", "minimum": 10 }] },
                                        "exclusive": { "oneOf": [{ "type": "string", "minLength": 2 }, { "type": "string", "pattern": "^a" }] },
                                        "blocked": { "type": "string", "not": { "enum": ["forbidden"] } },
                                        "tags": { "type": "array", "minItems": 1, "maxItems": 2, "uniqueItems": true, "items": { "type": "string", "minLength": 2 } },
                                        "metadata": {
                                            "type": "object",
                                            "properties": { "owner": { "type": "string" } },
                                            "additionalProperties": false
                                        },
                                        "labels": {
                                            "type": "object",
                                            "additionalProperties": { "type": "string", "minLength": 2 }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("validation", "memory://validation", spec)
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("validation"))
        .await
        .unwrap();

    let invalid = store
        .call_tool(
            store_instance_id("validation"),
            "createValidatedItem",
            serde_json::json!({
                "limit": "ten",
                "status": "archived",
                "body": { "tags": ["ok", 1] }
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(invalid.contains("query.limit must be an integer"));
    assert!(invalid.contains("query.status must match one of the declared enum values"));
    assert!(invalid.contains("body.name is required"));
    assert!(invalid.contains("body.tags[1] must be a string"));

    let invalid_constraints = store
        .call_tool(
            store_instance_id("validation"),
            "createValidatedItem",
            serde_json::json!({
                "limit": 0,
                "status": "draft",
                "request_id": "not-a-uuid",
                "filter_pattern": "[",
                "body": { "name": "x", "code": "abcde", "price": 0, "discount": 1, "quantity": 7, "publish_date": "2026-02-30", "updated_at": "2026-01-01 10:00:00", "callback": "ftp://example.test/hook", "resource_uri": "not a uri", "resource_ref": "bad ref space", "unicode_iri": "relative/路径", "unicode_ref": "bad ref space", "contact": "not-email", "host": "-bad.example", "ipv4": "999.0.0.1", "ipv6": "not-ipv6", "pointer": "/items/~2bad", "relative_pointer": "01/name", "composed": "no", "choice": true, "exclusive": "abc", "blocked": "forbidden", "tags": [], "metadata": { "extra": true }, "labels": { "color": "r" } }
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(invalid_constraints.contains("query.limit must be greater than or equal to 1"));
    assert!(invalid_constraints.contains("query.request_id must be a valid UUID"));
    assert!(invalid_constraints.contains("query.filter_pattern must be a valid regular expression"));
    assert!(invalid_constraints.contains("body.name length must be at least 3"));
    assert!(invalid_constraints.contains("body.name must match pattern ^item-[0-9]+$"));
    assert!(invalid_constraints.contains("body.code length must be at most 4"));
    assert!(invalid_constraints.contains("body.price must be greater than 0"));
    assert!(invalid_constraints.contains("body.discount must be less than 1"));
    assert!(invalid_constraints.contains("body.quantity must be a multiple of 5"));
    assert!(invalid_constraints.contains("body.publish_date must match date format YYYY-MM-DD"));
    assert!(invalid_constraints.contains("body.updated_at must match RFC3339 date-time format"));
    assert!(invalid_constraints.contains("body.callback must be a valid HTTP(S) URL"));
    assert!(invalid_constraints.contains("body.resource_uri must be a valid absolute URI"));
    assert!(invalid_constraints.contains("body.resource_ref must be a valid URI reference"));
    assert!(invalid_constraints.contains("body.unicode_iri must be a valid IRI"));
    assert!(invalid_constraints.contains("body.unicode_ref must be a valid IRI reference"));
    assert!(invalid_constraints.contains("body.contact must be a valid email address"));
    assert!(invalid_constraints.contains("body.host must be a valid hostname"));
    assert!(invalid_constraints.contains("body.ipv4 must be a valid IPv4 address"));
    assert!(invalid_constraints.contains("body.ipv6 must be a valid IPv6 address"));
    assert!(invalid_constraints.contains("body.pointer must be a valid JSON Pointer"));
    assert!(
        invalid_constraints.contains("body.relative_pointer must be a valid relative JSON Pointer")
    );
    assert!(invalid_constraints.contains("body.composed length must be at least 3"));
    assert!(invalid_constraints.contains("body.composed must match pattern ^ok-"));
    assert!(invalid_constraints.contains("body.choice must match at least one anyOf schema"));
    assert!(invalid_constraints.contains("body.exclusive must match exactly one oneOf schema"));
    assert!(invalid_constraints.contains("body.blocked must not match not schema"));
    assert!(invalid_constraints.contains("body.tags must contain at least 1 item(s)"));
    assert!(invalid_constraints.contains("body.metadata.extra is not allowed"));
    assert!(invalid_constraints.contains("body.labels.color length must be at least 2"));

    let too_many_items = store
        .call_tool(
            store_instance_id("validation"),
            "createValidatedItem",
            serde_json::json!({
                "limit": 21,
                "status": "draft",
                "request_id": "550e8400-e29b-41d4-a716-446655440000",
                "filter_pattern": "^item-[0-9]+$",
                "body": { "name": "item-123", "code": "abcd", "price": 1, "discount": 0.5, "quantity": 10, "publish_date": "2026-01-01", "updated_at": "2026-01-01T10:00:00Z", "callback": "https://example.test/hook", "resource_uri": "mcpstore://items/123", "resource_ref": "../items/123?view=full#meta", "unicode_iri": "https://例子.测试/路径", "unicode_ref": "../路径?键=值", "contact": "ops@example.test", "host": "api.example.test", "ipv4": "192.0.2.1", "ipv6": "2001:db8::1", "pointer": "/items/0/name", "relative_pointer": "0#", "composed": "ok-ready", "choice": "alpha", "exclusive": "zz", "blocked": "allowed", "tags": ["a", "bb", "cc"] }
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(too_many_items.contains("query.limit must be less than or equal to 20"));
    assert!(too_many_items.contains("body.tags must contain at most 2 item(s)"));
    assert!(too_many_items.contains("body.tags[0] length must be at least 2"));

    let duplicate_items = store
        .call_tool(
            store_instance_id("validation"),
            "createValidatedItem",
            serde_json::json!({
                "limit": 10,
                "status": "draft",
                "request_id": "550e8400-e29b-41d4-a716-446655440000",
                "filter_pattern": "^item-[0-9]+$",
                "body": { "name": "item-123", "code": "abcd", "price": 1, "discount": 0.5, "quantity": 10, "publish_date": "2026-01-01", "updated_at": "2026-01-01T10:00:00Z", "callback": "https://example.test/hook", "resource_uri": "mcpstore://items/123", "resource_ref": "../items/123?view=full#meta", "unicode_iri": "https://例子.测试/路径", "unicode_ref": "../路径?键=值", "contact": "ops@example.test", "host": "api.example.test", "ipv4": "192.0.2.1", "ipv6": "2001:db8::1", "pointer": "/items/0/name", "relative_pointer": "1/items/0", "composed": "ok-ready", "choice": "alpha", "exclusive": "zz", "blocked": "allowed", "tags": ["bb", "bb"] }
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(duplicate_items.contains("body.tags must contain unique items"));

    let call_result = store
        .call_tool(
            store_instance_id("validation"),
            "createValidatedItem",
            serde_json::json!({
                "limit": 10,
                "status": "draft",
                "request_id": "550e8400-e29b-41d4-a716-446655440000",
                "filter_pattern": "^item-[0-9]+$",
                "body": { "name": "item-123", "code": "abcd", "price": 1.5, "discount": 0.5, "quantity": 10, "publish_date": "2026-01-01", "updated_at": "2026-01-01T10:00:00Z", "callback": "https://example.test/hook", "resource_uri": "mcpstore://items/123", "resource_ref": "../items/123?view=full#meta", "unicode_iri": "https://例子.测试/路径", "unicode_ref": "../路径?键=值", "contact": "ops@example.test", "host": "api.example.test", "ipv4": "192.0.2.1", "ipv6": "2001:db8::1", "pointer": "/items/0/~1escaped~0name", "relative_pointer": "0/items/0", "composed": "ok-ready", "choice": 10, "exclusive": "zz", "blocked": "allowed", "tags": ["fruit"], "metadata": { "owner": "ops" }, "labels": { "color": "red" } }
            }),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
}

#[tokio::test]
async fn openapi_import_options_apply_security_to_tools_and_resources() {
    let base_url = spawn_openapi_auth_http_fixture().await;
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Secure Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "components": {
            "securitySchemes": {
                "ApiKeyAuth": { "type": "apiKey", "in": "header", "name": "x-api-key" }
            }
        },
        "security": [{ "ApiKeyAuth": [] }],
        "paths": {
            "/secure/items": {
                "get": { "operationId": "listSecureItems", "summary": "List secure items" },
                "post": { "operationId": "createSecureItem", "requestBody": { "required": true } }
            }
        }
    });

    let missing_auth_store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-missing-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    missing_auth_store
        .import_openapi_service_from_spec("secure", "memory://secure", spec.clone())
        .await
        .unwrap();
    missing_auth_store
        .connect_service(store_instance_id("secure"))
        .await
        .unwrap();
    assert!(missing_auth_store
        .call_tool(
            store_instance_id("secure"),
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("missing auth value"));

    let header_store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-header-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    header_store
        .import_openapi_service_from_spec_with_options(
            "secure",
            "memory://secure",
            spec.clone(),
            crate::openapi::OpenApiImportOptions {
                headers: HashMap::from([("x-api-key".to_string(), "secret".to_string())]),
                auth: serde_json::Map::new(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    header_store
        .connect_service(store_instance_id("secure"))
        .await
        .unwrap();
    assert!(header_store
        .call_tool(
            store_instance_id("secure"),
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .is_ok());

    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let result = store
        .import_openapi_service_from_spec_with_options(
            "secure",
            "memory://secure",
            spec,
            crate::openapi::OpenApiImportOptions {
                headers: HashMap::new(),
                auth: serde_json::json!({ "ApiKeyAuth": "secret" })
                    .as_object()
                    .unwrap()
                    .clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("secure"))
        .await
        .unwrap();
    assert!(result.security_schemes.contains_key("ApiKeyAuth"));
    assert_eq!(result.security.len(), 1);

    let call_result = store
        .call_tool(
            store_instance_id("secure"),
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);

    let resource = store
        .read_resource(
            store_instance_id("secure"),
            "openapi://secure/listSecureItems",
        )
        .await
        .unwrap();
    assert!(resource["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("secured"));
}

#[tokio::test]
async fn local_source_processes_control_requests() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("test-control-request-worker".to_string()),
    })
    .unwrap();
    store
        .cache()
        .put_event(
            CONTROL_REQUEST_EVENT_TYPE,
            "evt-add",
            serde_json::json!({
                "id": "evt-add",
                "type": "ServiceAddRequested",
                "payload": {
                    "service_name": "queued",
                    "config": stdio_config(),
                },
                "source": "onlydb",
                "created_at": 111,
                "dedup_key": "ServiceAddRequested:queued",
                "trace_id": "evt-add",
                "status": "queued",
            }),
        )
        .await
        .unwrap();

    let processed = store.process_control_requests().await.unwrap();
    assert_eq!(processed, 1);
    assert!(store
        .cache()
        .get_entity("service_definitions", "queued")
        .await
        .unwrap()
        .is_some());
    let event = store
        .cache()
        .get_event(CONTROL_REQUEST_EVENT_TYPE, "evt-add")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event["status"], serde_json::json!("applied"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn switch_cache_storage_migrates_runtime_cache() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store
        .add_service("svc", agent_only_config("agent-a"))
        .await
        .unwrap();
    let instance_id = instance_id("svc", agent_scope("agent-a"));

    let snapshot = store
        .switch_cache_storage(CacheStorage::Memory, None, None)
        .await
        .unwrap();
    assert!(snapshot.entities["service_definitions"].contains_key("svc"));
    assert!(snapshot.entities["service_instances"].contains_key(&instance_id.to_string()));
    assert!(snapshot.relations["agent_instances"].contains_key("agent-a"));
    assert!(snapshot.states["service_state"].contains_key(&instance_id.to_string()));

    assert!(store
        .cache()
        .get_entity("service_instances", &instance_id.to_string())
        .await
        .unwrap()
        .is_some());
    let agent_instances = store
        .list_scope_instances(&agent_scope("agent-a"))
        .await
        .unwrap();
    assert_eq!(agent_instances.len(), 1);
    assert_eq!(agent_instances[0].instance_id, instance_id);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn switch_cache_storage_updates_namespace() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("before-switch".to_string()),
    })
    .unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();

    let snapshot = store
        .switch_cache_storage(CacheStorage::Memory, None, Some("after-switch".to_string()))
        .await
        .unwrap();

    assert!(snapshot.entities["service_definitions"].contains_key("svc"));
    assert_eq!(store.namespace(), "after-switch");

    let inspect = store.cache_inspect().await.unwrap();
    assert_eq!(inspect["namespace"], serde_json::json!("after-switch"));
    let collections = inspect["collections"].as_array().unwrap();
    assert!(collections.iter().any(|value| {
        value
            .as_str()
            .map(|text| text.starts_with("after-switch:entity:service_definitions"))
            .unwrap_or(false)
    }));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn switch_cache_storage_preserves_concurrent_writes() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("concurrent-before".to_string()),
    })
    .unwrap();
    for index in 0..500 {
        store
            .cache()
            .put_entity(
                "clients",
                &format!("seed-{index}"),
                serde_json::json!({"index": index}),
            )
            .await
            .unwrap();
    }

    let writer_store = store.clone();
    let writer = async move {
        for index in 0..100 {
            writer_store
                .cache()
                .put_entity(
                    "clients",
                    &format!("live-{index}"),
                    serde_json::json!({"index": index}),
                )
                .await
                .unwrap();
            tokio::task::yield_now().await;
        }
    };
    let migration = store.switch_cache_storage(
        CacheStorage::Memory,
        None,
        Some("concurrent-after".to_string()),
    );
    let ((), result) = tokio::join!(writer, migration);
    result.unwrap();

    for index in 0..100 {
        assert!(store
            .cache()
            .get_entity("clients", &format!("live-{index}"))
            .await
            .unwrap()
            .is_some(), "missing live-{index}");
    }

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn cache_inspect_includes_session_collections() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("inspect-sessions".to_string()),
    })
    .unwrap();
    let session_key = "store:s1";

    store
        .cache()
        .put_entity(
            "sessions",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "session_id": "s1",
                "scope": "store",
                "agent_id": null,
                "created_at": 100,
                "updated_at": 100,
                "last_active": 100,
                "lease_seconds": null,
                "expires_at": null,
                "version": 1,
                "metadata": {}
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_relation(
            "session_services",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "services": [],
                "updated_at": 100,
                "version": 1
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_relation(
            "session_tools",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "mode": "allowlist",
                "tools": [],
                "updated_at": 100,
                "version": 1
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_state(
            "session_status",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "status": "active",
                "updated_at": 100,
                "version": 1,
                "reason": null
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_event(
            "session_events",
            "store:s1:0001",
            serde_json::json!({
                "session_key": session_key,
                "event_type": "create",
                "occurred_at": 100,
                "payload": {}
            }),
        )
        .await
        .unwrap();

    let inspect = store.cache_inspect().await.unwrap();
    assert_eq!(inspect["counts"]["entities"]["sessions"], 1);
    assert_eq!(inspect["counts"]["relations"]["session_services"], 1);
    assert_eq!(inspect["counts"]["relations"]["session_tools"], 1);
    assert_eq!(inspect["counts"]["states"]["session_status"], 1);
    assert_eq!(inspect["counts"]["events"]["session_events"], 1);
    let collections = inspect["collections"].as_array().unwrap();
    for suffix in [
        "entity:sessions",
        "relations:session_services",
        "relations:session_tools",
        "state:session_status",
        "event:session_events",
    ] {
        let expected = format!("inspect-sessions:{suffix}");
        assert!(collections
            .iter()
            .any(|value| value.as_str() == Some(expected.as_str())));
    }
    assert_eq!(inspect["request_metrics"]["available"], true);
    assert!(
        inspect["request_metrics"]["total_requests"]
            .as_u64()
            .unwrap()
            > 0
    );

    store.reset_cache_request_metrics().await.unwrap();
    let after_reset = store.cache_inspect().await.unwrap();
    assert_eq!(after_reset["request_metrics"]["total_requests"], 0);
    assert_eq!(after_reset["request_metrics"]["hits"], 0);
    assert_eq!(after_reset["request_metrics"]["misses"], 0);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn memory_cache_storage_writes_cache_layers_through_openkeyv() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-openkeyv-memory".to_string()),
    })
    .unwrap();

    store.add_service("svc", stdio_config()).await.unwrap();

    assert_eq!(
        store.current_cache_storage().await,
        CacheStorage::OpenKeyvMemory
    );
    assert!(store
        .cache()
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_some());
    assert!(store
        .cache()
        .get_state("service_state", &store_instance_id("svc").to_string())
        .await
        .unwrap()
        .is_some());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn switch_cache_storage_to_openkeyv_memory_migrates_runtime_cache() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();

    let snapshot = store
        .switch_cache_storage(CacheStorage::OpenKeyvMemory, None, None)
        .await
        .unwrap();

    assert!(snapshot.entities["service_definitions"].contains_key("svc"));
    assert_eq!(
        store.current_cache_storage().await,
        CacheStorage::OpenKeyvMemory
    );
    assert!(store
        .cache()
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_some());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn update_and_patch_service_update_runtime_cache() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-update-patch".to_string()),
    })
    .unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();

    let mut updated = stdio_config();
    updated.args = vec!["updated".to_string()];
    store.update_service("svc", updated).await.unwrap();
    let config = store
        .get_effective_config("svc", &store_scope())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config["args"], serde_json::json!(["updated"]));

    store
        .patch_service("svc", serde_json::json!({"description": "patched"}))
        .await
        .unwrap();
    let config = store
        .get_effective_config("svc", &store_scope())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config["description"], serde_json::json!("patched"));
}

#[tokio::test]
async fn event_history_and_cache_health_are_reported() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-event-health".to_string()),
    })
    .unwrap();

    store
        .publish_event("TEST_EVENT", serde_json::json!({"ok": true}), true)
        .await
        .unwrap();
    let history = store.event_history(10).await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].event_type, "TEST_EVENT");

    store.add_service("svc", stdio_config()).await.unwrap();
    let health = store.cache_health_check().await.unwrap();
    assert_eq!(health["backend"], serde_json::json!("openkeyv_memory"));
    assert!(health["entities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("service_definitions")));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn list_tools_uses_registry_without_transport_connection() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-list-tools-registry".to_string()),
    })
    .unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();

    let tools = store.list_tools(store_instance_id("svc")).await.unwrap();
    assert!(tools.is_empty());

    std::fs::remove_file(path).ok();
}

fn registry_tool(name: &str) -> crate::registry::ToolInfo {
    crate::registry::ToolInfo {
        name: name.to_string(),
        title: None,
        description: format!("{name} description"),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"},
                "debug": {"type": "boolean"}
            },
            "required": ["message"]
        }),
        output_schema: None,
        annotations: None,
        meta: None,
    }
}

async fn install_registry_tools(
    store: &MCPStore,
    instance_id: InstanceId,
    tools: Vec<crate::registry::ToolInfo>,
) {
    let mut instance = store.registry.find_instance(instance_id).await.unwrap();
    instance.tools = tools;
    store.registry.register_instance(instance).await;
}

#[tokio::test]
async fn tool_transform_rules_affect_only_the_addressed_instance_view() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    install_registry_tools(&store, instance_id, vec![registry_tool("echo")]).await;

    let rule = store
        .set_tool_transform(
            instance_id,
            "echo",
            crate::ToolTransformPatch::default()
                .with_display_name("say")
                .with_description("Send a message")
                .rename_argument("message", "text")
                .hide_argument("debug", false)
                .with_tag("friendly"),
        )
        .await
        .unwrap();
    assert_eq!(rule.instance_id, instance_id);
    assert_eq!(rule.service_name, "svc");
    assert_eq!(rule.scope, store_scope());
    assert_eq!(rule.tool_name, "echo");
    assert_eq!(rule.display_name.as_deref(), Some("say"));

    let entries = store
        .list_tool_entries_for_instance(instance_id)
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "say");
    assert_eq!(entries[0].tool_name, "echo");
    assert_eq!(entries[0].description, "Send a message");
    assert!(entries[0].input_schema["properties"].get("text").is_some());
    assert!(entries[0].input_schema["properties"]
        .get("message")
        .is_none());
    assert!(entries[0].input_schema["properties"].get("debug").is_none());

    assert_eq!(
        store
            .get_tool_transform(instance_id, "say")
            .await
            .unwrap()
            .unwrap()
            .tool_name,
        "echo"
    );
    assert_eq!(store.list_tool_transforms().await.unwrap().len(), 1);
    store
        .delete_tool_transform(instance_id, "say")
        .await
        .unwrap();
    assert!(store
        .get_tool_transform(instance_id, "echo")
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn rust_tool_transform_builders_create_instance_owned_rules() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    install_registry_tools(
        &store,
        instance_id,
        vec![
            registry_tool("friendly"),
            registry_tool("renamed"),
            registry_tool("validated"),
        ],
    )
    .await;

    let friendly = store
        .create_llm_friendly_tool_transform(
            instance_id,
            "friendly",
            Some("friendly_simple"),
            Some("Friendly tool"),
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(friendly.instance_id, instance_id);
    assert!(friendly.tags.contains(&"llm-friendly".to_string()));
    assert!(friendly
        .arguments
        .iter()
        .any(|argument| { argument.original_name == "debug" && argument.hidden }));
    assert!(friendly.safety_policy.is_some());

    let renamed = store
        .create_parameter_renamed_tool_transform(
            instance_id,
            "renamed",
            Some("renamed_simple"),
            &[("message", "text")],
        )
        .await
        .unwrap();
    assert_eq!(renamed.arguments[0].original_name, "message");
    assert_eq!(renamed.arguments[0].new_name.as_deref(), Some("text"));

    let validated = store
        .create_validated_tool_transform(
            instance_id,
            "validated",
            None,
            &[(
                "message",
                serde_json::json!({"type": "string", "minLength": 1}),
            )],
        )
        .await
        .unwrap();
    assert_eq!(
        validated.display_name.as_deref(),
        Some("validated_validated")
    );
    assert!(validated.arguments[0].validation_schema.is_some());
    assert_eq!(store.list_tool_transforms().await.unwrap().len(), 3);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn context_tool_visibility_reapplies_after_tool_refresh() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    install_registry_tools(
        &store,
        instance_id,
        vec![registry_tool("alpha"), registry_tool("beta")],
    )
    .await;

    store
        .set_context_tool_visibility(instance_id, vec!["alpha".to_string()])
        .await
        .unwrap();
    store
        .registry
        .replace_instance_tools(
            instance_id,
            vec![registry_tool("alpha"), registry_tool("gamma")],
        )
        .await;

    let policy = crate::agent::tool_visibility::EffectiveToolPolicy::resolve(&store, instance_id)
        .await
        .unwrap();
    assert_eq!(
        policy
            .available
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha"]
    );
    assert_eq!(
        policy
            .removed
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["gamma"]
    );
    assert!(policy.stale.is_empty());

    store
        .registry
        .replace_instance_tools(instance_id, vec![registry_tool("gamma")])
        .await;
    let policy = crate::agent::tool_visibility::EffectiveToolPolicy::resolve(&store, instance_id)
        .await
        .unwrap();
    assert!(policy.available.is_empty());
    assert_eq!(
        policy
            .removed
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["gamma"]
    );
    assert_eq!(policy.stale, vec!["alpha"]);

    store
        .clear_context_tool_visibility(instance_id)
        .await
        .unwrap();
    let policy = crate::agent::tool_visibility::EffectiveToolPolicy::resolve(&store, instance_id)
        .await
        .unwrap();
    assert_eq!(
        policy
            .available
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["gamma"]
    );
    assert!(policy.removed.is_empty());
    assert!(policy.stale.is_empty());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn context_tool_visibility_filters_tools_per_instance() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    install_registry_tools(
        &store,
        instance_id,
        vec![registry_tool("alpha"), registry_tool("beta")],
    )
    .await;

    let state = store
        .set_context_tool_visibility(instance_id, vec!["alpha".to_string()])
        .await
        .unwrap();
    assert_eq!(state.instance_id, instance_id);
    assert_eq!(state.service_name, "svc");
    assert_eq!(state.scope, store_scope());
    assert_eq!(state.tools.len(), 1);
    assert_eq!(state.tools[0].tool_name, "alpha");

    let available = store
        .list_tool_entries_for_instance_with_filter(instance_id, ToolVisibilityFilter::Available)
        .await
        .unwrap();
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].tool_name, "alpha");
    assert_eq!(
        store
            .list_tools_scoped(&store_scope())
            .await
            .unwrap()
            .iter()
            .filter_map(|tool| tool.get("tool_name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec!["alpha"]
    );
    store
        .ensure_context_tool_allowed(instance_id, "alpha")
        .await
        .unwrap();
    assert!(matches!(
        store.ensure_context_tool_allowed(instance_id, "beta").await,
        Err(StoreError::ToolNotAvailable { .. })
    ));
    assert!(matches!(
        store
            .call_tool(instance_id, "beta", serde_json::json!({}))
            .await,
        Err(StoreError::ToolNotAvailable { .. })
    ));

    let stale_state = store
        .set_context_tool_visibility(
            instance_id,
            vec!["alpha".to_string(), "removed".to_string()],
        )
        .await
        .unwrap();
    assert_eq!(
        stale_state
            .tools
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "removed"]
    );
    let policy = crate::agent::tool_visibility::EffectiveToolPolicy::resolve(&store, instance_id)
        .await
        .unwrap();
    assert_eq!(policy.available.len(), 1);
    assert_eq!(policy.removed.len(), 1);
    assert_eq!(policy.stale, vec!["removed"]);

    let removed = store
        .list_tool_entries_for_instance_with_filter(instance_id, ToolVisibilityFilter::Removed)
        .await
        .unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].tool_name, "beta");

    store
        .clear_context_tool_visibility(instance_id)
        .await
        .unwrap();
    let restored = store
        .list_tool_entries_for_instance_with_filter(instance_id, ToolVisibilityFilter::Available)
        .await
        .unwrap();
    assert_eq!(restored.len(), 2);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn tool_preferences_are_stored_by_instance_and_tool() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    install_registry_tools(&store, instance_id, vec![registry_tool("echo")]).await;

    let state = store
        .set_tool_preference(
            instance_id,
            "echo",
            "return_direct",
            serde_json::json!(true),
        )
        .await
        .unwrap();
    assert_eq!(state.context_key, "store");
    assert_eq!(state.instance_id, instance_id);
    assert_eq!(state.service_name, "svc");
    assert_eq!(state.scope, store_scope());
    assert_eq!(state.tool_name, "echo");
    assert_eq!(state.preferences["return_direct"], true);
    assert!(store
        .cache()
        .get_state("tool_preferences", &format!("store:{instance_id}:echo"),)
        .await
        .unwrap()
        .is_some());

    assert_eq!(
        store
            .get_tool_preference(instance_id, "echo", "return_direct")
            .await
            .unwrap(),
        Some(serde_json::json!(true))
    );
    store
        .set_tool_preference(
            instance_id,
            "echo",
            "adapter",
            serde_json::json!("langchain"),
        )
        .await
        .unwrap();
    let remaining = store
        .clear_tool_preference(instance_id, "echo", "return_direct")
        .await
        .unwrap()
        .unwrap();
    assert!(remaining.preferences.get("return_direct").is_none());
    assert_eq!(remaining.preferences["adapter"], "langchain");

    assert!(store
        .clear_tool_preference(instance_id, "echo", "adapter")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get_tool_preferences(instance_id, "echo")
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn connect_service_failure_uses_default_no_restart_policy() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-connect-failure-status".to_string()),
    })
    .unwrap();
    store
        .add_service("broken", broken_stdio_config())
        .await
        .unwrap();

    let err = store
        .connect_service(store_instance_id("broken"))
        .await
        .unwrap_err()
        .to_string();
    let state = store
        .state_manager
        .get(store_instance_id("broken"))
        .await
        .unwrap()
        .unwrap();
    let service = store
        .find_instance(store_instance_id("broken"))
        .await
        .unwrap();

    assert!(err.contains("Connection failed"));
    assert_eq!(state.phase, crate::state::RuntimePhase::Stopped);
    assert_eq!(state.health, crate::state::HealthState::Unhealthy);
    assert!(matches!(
        state.recovery,
        crate::state::RecoveryState::Exhausted { attempts: 1 }
    ));
    assert!(state.failure.is_some());
    assert_eq!(service.applied_config_revision, None);
    assert!(store
        .cache()
        .get_entity(
            "service_instances",
            &store_instance_id("broken").to_string(),
        )
        .await
        .unwrap()
        .unwrap()["applied_config_revision"]
        .is_null());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn connect_service_accepts_server_without_tools_capability() {
    #[derive(Clone)]
    struct ResourceOnlyServer;

    impl ServerHandler for ResourceOnlyServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::builder().enable_resources().build())
        }
    }

    let service: StreamableHttpService<ResourceOnlyServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(ResourceOnlyServer),
            Default::default(),
            StreamableHttpServerConfig::default().with_sse_keep_alive(None),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().nest_service("/mcp", service))
            .await
            .unwrap();
    });

    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-connect-without-tools".to_string()),
    })
    .unwrap();
    let config = ServerConfig {
        url: Some(format!("http://{address}/mcp")),
        transport: Some("streamable-http".to_string()),
        ..ServerConfig::default()
    };
    store.add_service("resource-only", config).await.unwrap();

    let instance_id = store_instance_id("resource-only");
    store.connect_service(instance_id).await.unwrap();

    let instance = store.find_instance(instance_id).await.unwrap();
    assert_eq!(
        store
            .state_manager
            .get(instance_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        crate::state::RuntimePhase::Running
    );
    assert!(instance.tools.is_empty());
    let metadata = store
        .mcp_server_metadata(instance_id)
        .await
        .unwrap()
        .unwrap();
    assert!(metadata.capabilities.resources);
    assert!(!metadata.capabilities.tools);

    store.disconnect_service(instance_id).await.unwrap();
    server.abort();
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn resource_subscriptions_are_restored_after_reconnect() {
    #[derive(Clone)]
    struct SubscriptionServer(Arc<AtomicUsize>);

    impl ServerHandler for SubscriptionServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(
                ServerCapabilities::builder()
                    .enable_resources()
                    .enable_resources_subscribe()
                    .build(),
            )
        }

        async fn subscribe(
            &self,
            _request: rmcp::model::SubscribeRequestParams,
            _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
        ) -> std::result::Result<(), rmcp::ErrorData> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let subscribe_count = Arc::new(AtomicUsize::new(0));
    let server_count = subscribe_count.clone();
    let service: StreamableHttpService<SubscriptionServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(SubscriptionServer(server_count.clone())),
            Default::default(),
            StreamableHttpServerConfig::default().with_sse_keep_alive(None),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().nest_service("/mcp", service))
            .await
            .unwrap();
    });

    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-resource-subscription-reconnect".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "subscriptions",
            ServerConfig {
                url: Some(format!("http://{address}/mcp")),
                transport: Some("streamable-http".to_string()),
                ..ServerConfig::default()
            },
        )
        .await
        .unwrap();
    let instance_id = store_instance_id("subscriptions");

    store.connect_service(instance_id).await.unwrap();
    store
        .subscribe_resource_updates(instance_id, "fixture://watched")
        .await
        .unwrap();
    assert_eq!(subscribe_count.load(Ordering::SeqCst), 1);

    store.disconnect_service(instance_id).await.unwrap();
    store.connect_service(instance_id).await.unwrap();
    assert_eq!(subscribe_count.load(Ordering::SeqCst), 2);

    store.disconnect_service(instance_id).await.unwrap();
    server.abort();
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn connect_service_times_out_hanging_stdio_startup() {
    let fixture_dir =
        std::env::temp_dir().join(format!("mcpstore-connect-timeout-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let path = fixture_dir.join("mcp.json").to_string_lossy().to_string();
    let app_path = fixture_dir.join("config.toml");
    std::fs::write(&app_path, "[health_check]\nstartup_timeout = 1\n").unwrap();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-connect-timeout-status".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "hanging",
            config_with_lifecycle(
                hanging_stdio_config(),
                None,
                Some(crate::config::RestartPolicy {
                    kind: crate::config::RestartPolicyKind::OnFailure,
                    max_retries: None,
                }),
            ),
        )
        .await
        .unwrap();

    let err = store
        .connect_service(store_instance_id("hanging"))
        .await
        .unwrap_err()
        .to_string();
    let state = store
        .state_manager
        .get(store_instance_id("hanging"))
        .await
        .unwrap()
        .unwrap();

    assert!(
        err.contains("Service instance connection timed out"),
        "{err}"
    );
    assert_eq!(state.phase, crate::state::RuntimePhase::Stopped);
    assert_eq!(state.health, crate::state::HealthState::Unhealthy);
    assert!(matches!(
        state.recovery,
        crate::state::RecoveryState::Waiting { attempt: 1, .. }
    ));

    std::fs::remove_dir_all(fixture_dir).ok();
}

#[tokio::test]
async fn automatic_retry_respects_backoff_and_enters_half_open_when_due() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-retry-backoff".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "broken",
            broken_stdio_config_with_restart_policy(crate::config::RestartPolicy {
                kind: crate::config::RestartPolicyKind::OnFailure,
                max_retries: None,
            }),
        )
        .await
        .unwrap();
    store
        .connect_service(store_instance_id("broken"))
        .await
        .unwrap_err();

    let blocked = store
        .connect_service_internal(store_instance_id("broken"), true)
        .await
        .unwrap_err()
        .to_string();
    assert!(blocked.contains("backoff active"));

    let state = store
        .state_manager
        .get(store_instance_id("broken"))
        .await
        .unwrap()
        .unwrap();
    let attempt = match state.recovery {
        crate::state::RecoveryState::Waiting { attempt, .. } => attempt,
        other => panic!("expected waiting recovery, got {other:?}"),
    };
    let transitioned = store
        .state_manager
        .dispatch(
            store_instance_id("broken"),
            crate::state::ServiceStateEvent::RecoveryProbeStarted { attempt },
            MCPStore::now_timestamp(),
        )
        .await
        .unwrap();
    assert!(matches!(
        transitioned.recovery,
        crate::state::RecoveryState::Probing { .. }
    ));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn manual_startup_policy_blocks_implicit_connect() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-manual-startup-policy".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "manual",
            stdio_config_with_lifecycle(Some(crate::config::StartupPolicy::Manual), None),
        )
        .await
        .unwrap();

    let err = store
        .ensure_instance_connected(store_instance_id("manual"))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("startup_policy=manual"));
    assert!(!store.pool.is_connected(store_instance_id("manual")).await);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn on_failure_max_retries_caps_lifecycle_restart_attempts() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-on-failure-max-retries".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "broken",
            broken_stdio_config_with_restart_policy(crate::config::RestartPolicy {
                kind: crate::config::RestartPolicyKind::OnFailure,
                max_retries: Some(1),
            }),
        )
        .await
        .unwrap();

    store
        .connect_service(store_instance_id("broken"))
        .await
        .unwrap_err();
    let first = store
        .state_manager
        .get(store_instance_id("broken"))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        first.recovery,
        crate::state::RecoveryState::Waiting { attempt: 1, .. }
    ));

    store
        .state_manager
        .dispatch(
            store_instance_id("broken"),
            crate::state::ServiceStateEvent::RecoveryProbeStarted { attempt: 1 },
            MCPStore::now_timestamp(),
        )
        .await
        .unwrap();
    store
        .connect_service_internal(store_instance_id("broken"), true)
        .await
        .unwrap_err();
    let second = store
        .state_manager
        .get(store_instance_id("broken"))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        second.recovery,
        crate::state::RecoveryState::Exhausted { attempts: 2 }
    ));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn oauth_service_state_and_api_response_do_not_expose_secrets() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-oauth-status-projection".to_string()),
    })
    .unwrap();
    store
        .add_service("protected", oauth_http_config())
        .await
        .unwrap();
    let instance_id = store_instance_id("protected");

    assert_eq!(
        store.auth_status(instance_id).await,
        crate::auth::AuthStatus::Unauthenticated
    );
    let response = store.service_info_scoped(instance_id).await.unwrap();
    let response = serde_json::to_string(&response).unwrap();
    for secret in [
        "access-secret-value",
        "refresh-secret-value",
        "client-secret-value",
        "pkce-secret-value",
        "csrf-secret-value",
    ] {
        assert!(!response.contains(secret));
    }

    store
        .auth_coordinator
        .set_status(instance_id, crate::auth::AuthStatus::Authenticated)
        .await;
    store.remove_service("protected").await.unwrap();
    assert!(store
        .state_manager
        .get(instance_id)
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn authorization_callback_uri_only_exposes_authorization_code_redirect_uri() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-oauth-callback-uri".to_string()),
    })
    .unwrap();
    store
        .add_service("protected", oauth_http_config())
        .await
        .unwrap();
    store.add_service("plain", stdio_config()).await.unwrap();

    assert_eq!(
        store
            .authorization_callback_uri(store_instance_id("protected"))
            .await
            .unwrap()
            .as_deref(),
        Some("http://127.0.0.1:8787/oauth/callback")
    );
    assert_eq!(
        store
            .authorization_callback_uri(store_instance_id("plain"))
            .await
            .unwrap(),
        None
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn auth_required_does_not_enter_retry_or_circuit_breaker_state() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-auth-required-lifecycle".to_string()),
    })
    .unwrap();
    store
        .add_service("protected", stdio_config())
        .await
        .unwrap();
    let instance_id = store_instance_id("protected");
    let error = crate::transport::TransportError::AuthRequired(crate::auth::AuthRequired {
        instance_id,
        flow: crate::auth::AuthFlow::AuthorizationCode,
        scopes: vec!["tools.read".to_string()],
    });

    store
        .record_transport_failure(instance_id, &error, "Connection failed")
        .await
        .unwrap();

    let state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(state.phase, crate::state::RuntimePhase::Stopped);
    assert_eq!(state.recovery, crate::state::RecoveryState::Idle);
    assert_eq!(state.failure, None);
    assert_eq!(
        store.auth_status(instance_id).await,
        crate::auth::AuthStatus::Unauthenticated
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn insufficient_scope_does_not_enter_retry_or_circuit_breaker_state() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-insufficient-scope-lifecycle".to_string()),
    })
    .unwrap();
    store
        .add_service("protected", oauth_http_config())
        .await
        .unwrap();
    let instance_id = store_instance_id("protected");
    let error = crate::transport::TransportError::InsufficientScope {
        instance_id,
        required_scope: Some("resources.read tools.call".to_string()),
    };

    store
        .record_transport_failure(instance_id, &error, "Connection failed")
        .await
        .unwrap();

    let state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(state.phase, crate::state::RuntimePhase::Stopped);
    assert_eq!(state.recovery, crate::state::RecoveryState::Idle);
    assert_eq!(state.failure, None);
    assert_eq!(
        store.auth_status(instance_id).await,
        crate::auth::AuthStatus::ScopeUpgradeRequired
    );
    assert_eq!(
        store
            .auth_status_view(instance_id)
            .await
            .unwrap()
            .required_scope
            .as_deref(),
        Some("resources.read tools.call")
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn successful_health_check_records_canonical_health() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-health-observation".to_string()),
    })
    .unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let instance_id = store_instance_id("svc");
    store
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::StartRequested,
            MCPStore::now_timestamp(),
        )
        .await
        .unwrap();
    store
        .state_manager
        .dispatch(
            instance_id,
            crate::state::ServiceStateEvent::TransportConnected,
            MCPStore::now_timestamp(),
        )
        .await
        .unwrap();

    let observed = store
        .record_health_check_result(instance_id, true, Some(12.0), None)
        .await
        .unwrap();

    assert_eq!(observed.health, crate::state::HealthState::Healthy);
    assert_eq!(observed.recovery, crate::state::RecoveryState::Idle);
    assert_eq!(observed.failure, None);
    assert_eq!(observed.health_metrics.latency_p95_ms, Some(12.0));
    assert_eq!(observed.health_metrics.latency_p99_ms, Some(12.0));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn export_instance_config_projects_third_party_config_without_mcpstore_extension() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-config-format-projection".to_string()),
    })
    .unwrap();
    store
        .add_service(
            "svc",
            stdio_config_with_lifecycle(
                Some(crate::config::StartupPolicy::OnStoreStart),
                Some(crate::config::RestartPolicy {
                    kind: crate::config::RestartPolicyKind::OnFailure,
                    max_retries: Some(3),
                }),
            ),
        )
        .await
        .unwrap();

    let native = store.show_config().await.unwrap();
    assert!(native["mcpServers"]["svc"].get("_mcpstore").is_some());

    let claude = store
        .export_instance_config(
            store_instance_id("svc"),
            crate::config_formats::ConfigFormat::Claude,
        )
        .await
        .unwrap();
    assert!(claude["mcpServers"]["svc"].get("_mcpstore").is_none());
    assert_eq!(
        claude["mcpServers"]["svc"]["command"],
        serde_json::json!("echo")
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn db_load_does_not_rewrite_cached_agent_relations() {
    let source_path = temp_config_path();
    let source = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(source_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some(format!("test-db-load-seed-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    source
        .add_service("svc", agent_only_config("agent-a"))
        .await
        .unwrap();
    let relation_before = source
        .cache()
        .get_relation("agent_instances", "agent-a")
        .await
        .unwrap()
        .unwrap();

    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some(format!("test-db-load-readonly-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    copy_cache_snapshot(&source, &store).await;

    store.load_from_db().await.unwrap();

    let relation_after = store
        .cache()
        .get_relation("agent_instances", "agent-a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(relation_after, relation_before);

    std::fs::remove_file(source_path).ok();
}

#[tokio::test]
async fn tool_change_diff_reports_added_removed_and_updated_tools() {
    let old_tools = vec![
        crate::registry::ToolInfo {
            name: "keep".to_string(),
            title: None,
            description: "old description".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: None,
            meta: None,
        },
        crate::registry::ToolInfo {
            name: "remove".to_string(),
            title: None,
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: None,
            meta: None,
        },
    ];
    let new_tools = vec![
        crate::registry::ToolInfo {
            name: "add".to_string(),
            title: None,
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: None,
            meta: None,
        },
        crate::registry::ToolInfo {
            name: "keep".to_string(),
            title: None,
            description: "new description".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: None,
            meta: None,
        },
    ];

    let (added, removed, updated, count) =
        MCPStore::diff_tool_infos_for_test(&old_tools, &new_tools);

    assert_eq!(added, vec!["add"]);
    assert_eq!(removed, vec!["remove"]);
    assert_eq!(updated, vec!["keep"]);
    assert_eq!(count, 3);
}

mod scoped_contract {
    use super::*;

    use std::collections::HashMap;

    use serde_json::{json, Map, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::cache::models::ServiceDefinitionEntity;
    use crate::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor};
    use crate::identity::{InstanceId, ScopeRef, ServiceInstanceKey};
    use crate::registry::{ConfigRevision, ToolInfo};
    use crate::{CreateSessionRequest, SessionToolSelection, ToolTransformPatch};

    fn temp_config_path() -> String {
        std::env::temp_dir()
            .join(format!("mcpstore-store-{}.json", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string()
    }

    fn store_options(path: Option<String>) -> StoreOptions {
        StoreOptions {
            config_path: path,
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(format!("store-tests-{}", uuid::Uuid::new_v4())),
        }
    }

    fn store_scope() -> ScopeRef {
        ScopeRef::Store
    }

    fn agent_scope(agent_id: &str) -> ScopeRef {
        ScopeRef::Agent {
            agent_id: agent_id.to_string(),
        }
    }

    fn instance_id(service_name: &str, scope: ScopeRef) -> InstanceId {
        ServiceInstanceKey::new(service_name, scope).instance_id()
    }

    fn descriptor(config: Value) -> ScopeDescriptor {
        ScopeDescriptor {
            config: config.as_object().cloned().unwrap_or_default(),
            lifecycle: None,
            revision: 1,
        }
    }

    fn native_config(scopes: ScopeDeclarations) -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("fixture-command".to_string()),
            args: vec!["--base".to_string()],
            env: HashMap::from([
                ("SHARED".to_string(), "base".to_string()),
                ("REMOVE_ME".to_string(), "base".to_string()),
            ]),
            headers: HashMap::from([
                ("x-shared".to_string(), "base".to_string()),
                ("x-remove".to_string(), "base".to_string()),
            ]),
            auth: Default::default(),
            transport: Some("stdio".to_string()),
            working_dir: Some("/base".to_string()),
            description: Some("base definition".to_string()),
            mcpstore: Some(McpStoreExtension {
                scopes,
                lifecycle: None,
                revision: 1,
                extra: Map::new(),
            }),
            extra: Map::from_iter([(
                "nested".to_string(),
                json!({
                    "shared": "base",
                    "remove": "base",
                    "keep": "base"
                }),
            )]),
        }
    }

    fn tool(name: &str) -> ToolInfo {
        ToolInfo {
            name: name.to_string(),
            title: None,
            description: format!("{name} description"),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "debug": {"type": "boolean"}
                },
                "required": ["text"]
            }),
            output_schema: None,
            annotations: None,
            meta: None,
        }
    }

    async fn install_tool(store: &MCPStore, instance_id: InstanceId, tool_info: ToolInfo) {
        let mut instance = store.find_instance(instance_id).await.unwrap();
        instance.tools = vec![tool_info];
        store.registry.register_instance(instance).await;
    }

    async fn spawn_openapi_auth_fixture() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = vec![0; 8192];
                    let Ok(size) = socket.read(&mut buffer).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buffer[..size]);
                    let authorized = request
                        .lines()
                        .any(|line| line.eq_ignore_ascii_case("x-api-key: applied-secret"));
                    let first_line = request.lines().next().unwrap_or_default();
                    let transformed_body = request.contains(r#"{"text":"hello"}"#)
                        && !request.contains(r#""payload""#);
                    let (status, body) = if authorized
                        && (first_line.starts_with("GET /secure/items ")
                            || first_line.starts_with("POST /secure/items ")
                            || (first_line.starts_with("POST /echo ") && transformed_body))
                    {
                        ("200 OK", json!({"items": ["secured"], "received": true}))
                    } else {
                        ("401 Unauthorized", json!({"error": "unauthorized"}))
                    };
                    let body = body.to_string();
                    let response = format!(
                        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await.ok();
                });
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn third_party_config_materializes_one_store_instance() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        let mut config = native_config(ScopeDeclarations::default());
        config.mcpstore = None;

        store.add_service("svc", config).await.unwrap();

        let expected_id = instance_id("svc", store_scope());
        let instances = store.list_instances().await;
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].instance_id, expected_id);
        assert_eq!(instances[0].scope, ScopeRef::Store);
        assert!(store
            .cache()
            .get_entity("service_definitions", "svc")
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_entity("service_instances", &expected_id.to_string())
            .await
            .unwrap()
            .is_some());

        let saved = store.show_config_entry().await.unwrap();
        let extension = saved.mcp_servers["svc"].mcpstore.as_ref().unwrap();
        assert!(extension.scopes.store.is_some());
        assert!(extension.scopes.agents.is_empty());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn identical_effective_configs_still_create_isolated_scope_instances() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
                ("agent-3".to_string(), ScopeDescriptor::default()),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_1_id = instance_id("svc", agent_scope("agent-1"));
        let agent_2_id = instance_id("svc", agent_scope("agent-2"));
        let agent_3_id = instance_id("svc", agent_scope("agent-3"));
        assert_eq!(store.list_instances().await.len(), 4);
        assert_ne!(store_id, agent_1_id);
        assert_ne!(store_id, agent_2_id);
        assert_ne!(store_id, agent_3_id);
        assert_ne!(agent_1_id, agent_2_id);
        assert_ne!(agent_1_id, agent_3_id);
        assert_ne!(agent_2_id, agent_3_id);

        let store_instance = store.find_instance(store_id).await.unwrap();
        let agent_1 = store.find_instance(agent_1_id).await.unwrap();
        let agent_2 = store.find_instance(agent_2_id).await.unwrap();
        let agent_3 = store.find_instance(agent_3_id).await.unwrap();
        assert_eq!(store_instance.effective_config, agent_1.effective_config);
        assert_eq!(store_instance.effective_config, agent_2.effective_config);
        assert_eq!(store_instance.effective_config, agent_3.effective_config);
        assert_eq!(agent_1.scope, agent_scope("agent-1"));
        assert_eq!(agent_2.scope, agent_scope("agent-2"));
        assert_eq!(agent_3.scope, agent_scope("agent-3"));

        let relation = store
            .cache()
            .get_relation("agent_instances", "agent-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(relation["instances"][0]["instance_id"], json!(agent_1_id));
        assert_ne!(relation["instances"][0]["instance_id"], json!(agent_2_id));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn instance_ids_ignore_config_changes_and_agent_declaration_order() {
        let first_path = temp_config_path();
        let second_path = temp_config_path();

        let mut first_agents = HashMap::new();
        first_agents.insert(
            "agent-1".to_string(),
            descriptor(json!({"env": {"AGENT": "one"}})),
        );
        first_agents.insert(
            "agent-2".to_string(),
            descriptor(json!({"env": {"AGENT": "two"}})),
        );
        let mut first_config = native_config(ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: first_agents,
        });
        first_config.command = Some("first-command".to_string());

        let mut second_agents = HashMap::new();
        second_agents.insert(
            "agent-2".to_string(),
            descriptor(json!({"env": {"AGENT": "changed-two"}})),
        );
        second_agents.insert(
            "agent-1".to_string(),
            descriptor(json!({"env": {"AGENT": "changed-one"}})),
        );
        let mut second_config = native_config(ScopeDeclarations {
            store: Some(descriptor(json!({"args": ["--store-changed"]}))),
            agents: second_agents,
        });
        second_config.command = Some("second-command".to_string());

        let first = MCPStore::setup_with_options(store_options(Some(first_path.clone()))).unwrap();
        first.add_service("svc", first_config).await.unwrap();
        let second =
            MCPStore::setup_with_options(store_options(Some(second_path.clone()))).unwrap();
        second.add_service("svc", second_config).await.unwrap();

        let mut first_ids = first
            .list_instances()
            .await
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect::<Vec<_>>();
        let mut second_ids = second
            .list_instances()
            .await
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect::<Vec<_>>();
        first_ids.sort();
        second_ids.sort();
        assert_eq!(first_ids, second_ids);
        let mut expected_ids = vec![
            instance_id("svc", store_scope()),
            instance_id("svc", agent_scope("agent-1")),
            instance_id("svc", agent_scope("agent-2")),
        ];
        expected_ids.sort();
        assert_eq!(first_ids, expected_ids);

        std::fs::remove_file(first_path).ok();
        std::fs::remove_file(second_path).ok();
    }

    #[tokio::test]
    async fn retry_state_is_isolated_between_sibling_instances() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([("agent-1".to_string(), ScopeDescriptor::default())]),
        };
        let mut config = native_config(scopes);
        config.command = Some("__mcpstore_missing_binary__".to_string());
        config.args.clear();
        config.mcpstore.as_mut().unwrap().lifecycle = Some(crate::config::ServiceLifecycleConfig {
            startup_policy: None,
            restart_policy: Some(crate::config::RestartPolicy {
                kind: crate::config::RestartPolicyKind::OnFailure,
                max_retries: None,
            }),
        });
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store.add_service("svc", config).await.unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_id = instance_id("svc", agent_scope("agent-1"));
        store.connect_service(store_id).await.unwrap_err();

        let failed = store.state_manager.get(store_id).await.unwrap().unwrap();
        assert_eq!(failed.health, crate::state::HealthState::Unhealthy);
        assert!(matches!(
            failed.recovery,
            crate::state::RecoveryState::Waiting { attempt: 1, .. }
        ));
        assert!(failed.failure.is_some());

        let sibling = store.state_manager.get(agent_id).await.unwrap().unwrap();
        assert_eq!(sibling.desired, crate::state::DesiredState::Stopped);
        assert_eq!(sibling.phase, crate::state::RuntimePhase::Stopped);
        assert_eq!(sibling.health, crate::state::HealthState::Unknown);
        assert_eq!(sibling.recovery, crate::state::RecoveryState::Idle);
        assert_eq!(sibling.failure, None);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn scope_override_recursively_merges_and_null_deletes_inherited_fields() {
        let path = temp_config_path();
        let agent = agent_scope("agent-1");
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([(
                "agent-1".to_string(),
                descriptor(json!({
                    "command": "agent-command",
                    "args": ["--agent"],
                    "env": {
                        "SHARED": "agent",
                        "REMOVE_ME": null,
                        "AGENT_ONLY": "yes"
                    },
                    "headers": {
                        "x-shared": "agent",
                        "x-remove": null
                    },
                    "workingDir": null,
                    "nested": {
                        "shared": "agent",
                        "remove": null,
                        "agent": "only"
                    }
                })),
            )]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let effective = store
            .get_effective_config("svc", &agent)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(effective["command"], "agent-command");
        assert_eq!(effective["args"], json!(["--agent"]));
        assert_eq!(effective["env"]["SHARED"], "agent");
        assert_eq!(effective["env"]["AGENT_ONLY"], "yes");
        assert!(effective["env"].get("REMOVE_ME").is_none());
        assert_eq!(effective["headers"]["x-shared"], "agent");
        assert!(effective["headers"].get("x-remove").is_none());
        assert!(effective.get("workingDir").is_none());
        assert_eq!(effective["nested"]["shared"], "agent");
        assert_eq!(effective["nested"]["keep"], "base");
        assert_eq!(effective["nested"]["agent"], "only");
        assert!(effective["nested"].get("remove").is_none());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn config_update_preserves_observed_state_and_applied_revision() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([(
                "agent-1".to_string(),
                descriptor(json!({"env": {"AGENT": "one"}})),
            )]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        let original = native_config(scopes);
        store.add_service("svc", original.clone()).await.unwrap();

        let store_instance_id = instance_id("svc", store_scope());
        let mut connected = store.find_instance(store_instance_id).await.unwrap();
        let applied_revision = connected.config_revision;
        connected.tools = vec![tool("echo")];
        connected.applied_config_revision = Some(applied_revision);
        store.registry.register_instance(connected).await;

        let mut updated = original;
        updated.mcpstore = None;
        updated.command = Some("changed-command".to_string());
        updated.args = vec!["--changed".to_string()];
        store.update_service("svc", updated.clone()).await.unwrap();

        let instance = store.find_instance(store_instance_id).await.unwrap();
        assert_eq!(instance.tools, vec![tool("echo")]);
        assert_eq!(
            instance.applied_config_revision,
            Some(ConfigRevision {
                base_revision: 1,
                scope_revision: 1,
            })
        );
        assert_eq!(instance.config_revision.base_revision, 2);
        assert_eq!(instance.config_revision.scope_revision, 1);
        assert!(instance.restart_required());
        assert_eq!(instance.command.as_deref(), Some("changed-command"));

        let agent_instance_id = instance_id("svc", agent_scope("agent-1"));
        let agent_instance = store.find_instance(agent_instance_id).await.unwrap();
        assert_eq!(agent_instance.scope, agent_scope("agent-1"));
        assert_eq!(agent_instance.effective_config["env"]["AGENT"], "one");

        let definition = store.registry.find_definition("svc").await.unwrap();
        assert!(definition.scopes.store.is_some());
        assert!(definition.scopes.agents.contains_key("agent-1"));
        assert_eq!(definition.base_revision, 2);

        store.update_service("svc", updated).await.unwrap();
        let unchanged = store.registry.find_definition("svc").await.unwrap();
        assert_eq!(unchanged.base_revision, 2);
        assert!(unchanged.scopes.store.is_some());
        assert!(unchanged.scopes.agents.contains_key("agent-1"));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn base_and_scope_updates_recompute_only_owned_instances() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                (
                    "agent-1".to_string(),
                    descriptor(json!({"env": {"AGENT": "one"}})),
                ),
                (
                    "agent-2".to_string(),
                    descriptor(json!({"env": {"AGENT": "two"}})),
                ),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        let original = native_config(scopes);
        store.add_service("svc", original.clone()).await.unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_1_id = instance_id("svc", agent_scope("agent-1"));
        let agent_2_id = instance_id("svc", agent_scope("agent-2"));

        let mut updated = original;
        updated.mcpstore = None;
        updated.command = Some("changed-command".to_string());
        updated
            .env
            .insert("SHARED".to_string(), "changed-base".to_string());
        store.update_service("svc", updated).await.unwrap();

        let store_after_base = store.find_instance(store_id).await.unwrap();
        let agent_1_after_base = store.find_instance(agent_1_id).await.unwrap();
        let agent_2_after_base = store.find_instance(agent_2_id).await.unwrap();
        for instance in [&store_after_base, &agent_1_after_base, &agent_2_after_base] {
            assert_eq!(instance.command.as_deref(), Some("changed-command"));
            assert_eq!(instance.effective_config["env"]["SHARED"], "changed-base");
            assert_eq!(instance.config_revision.base_revision, 2);
            assert_eq!(instance.config_revision.scope_revision, 1);
        }
        assert_eq!(agent_1_after_base.effective_config["env"]["AGENT"], "one");
        assert_eq!(agent_2_after_base.effective_config["env"]["AGENT"], "two");

        store
            .declare_service_scope(
                "svc",
                &agent_scope("agent-1"),
                descriptor(json!({"env": {"AGENT": "changed-one"}})),
            )
            .await
            .unwrap();

        let agent_1_after_scope = store.find_instance(agent_1_id).await.unwrap();
        assert_eq!(agent_1_after_scope.config_revision.base_revision, 2);
        assert_eq!(agent_1_after_scope.config_revision.scope_revision, 2);
        assert_eq!(
            agent_1_after_scope.effective_config["env"]["AGENT"],
            "changed-one"
        );
        assert_eq!(
            store.find_instance(store_id).await.unwrap(),
            store_after_base
        );
        assert_eq!(
            store.find_instance(agent_2_id).await.unwrap(),
            agent_2_after_base
        );

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn config_update_rejects_mcpstore_extension() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(ScopeDeclarations::store_only()))
            .await
            .unwrap();

        let error = store
            .update_service("svc", native_config(ScopeDeclarations::default()))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("Use scope APIs"));
        let definition = store.registry.find_definition("svc").await.unwrap();
        assert_eq!(definition.base_revision, 1);
        assert!(definition.scopes.store.is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn startup_preserves_canonical_stopped_state() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(ScopeDeclarations::store_only()))
            .await
            .unwrap();
        let instance_id = instance_id("svc", store_scope());

        store
            .state_manager
            .dispatch(
                instance_id,
                crate::state::ServiceStateEvent::StartRequested,
                40,
            )
            .await
            .unwrap();
        store
            .state_manager
            .dispatch(
                instance_id,
                crate::state::ServiceStateEvent::TransportConnected,
                41,
            )
            .await
            .unwrap();
        store
            .state_manager
            .dispatch(
                instance_id,
                crate::state::ServiceStateEvent::StopRequested,
                42,
            )
            .await
            .unwrap();
        let observed = store
            .state_manager
            .dispatch(
                instance_id,
                crate::state::ServiceStateEvent::TransportStopped,
                43,
            )
            .await
            .unwrap();

        let mut runtime_instance = store.find_instance(instance_id).await.unwrap();
        runtime_instance.tools = vec![tool("echo")];
        runtime_instance.applied_config_revision = Some(runtime_instance.config_revision);
        store.registry.register_instance(runtime_instance).await;

        store.load_from_config().await.unwrap();

        let rebuilt = store.state_manager.get(instance_id).await.unwrap().unwrap();
        assert_eq!(rebuilt.desired, observed.desired);
        assert_eq!(rebuilt.phase, observed.phase);
        assert_eq!(rebuilt.health, observed.health);
        assert_eq!(rebuilt.recovery, observed.recovery);
        assert_eq!(rebuilt.auth, observed.auth);
        assert_eq!(rebuilt.tools, observed.tools);
        assert_eq!(rebuilt.failure, observed.failure);
        let instance = store.find_instance(instance_id).await.unwrap();
        assert!(instance.tools.is_empty());
        assert_eq!(instance.applied_config_revision, None);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn declare_scope_owns_revision_and_only_increments_for_actual_changes() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(ScopeDeclarations::store_only()))
            .await
            .unwrap();
        let scope = agent_scope("agent-1");

        let mut initial = descriptor(json!({"env": {"MODE": "agent"}}));
        initial.revision = 900;
        let instance_id = store
            .declare_service_scope("svc", &scope, initial.clone())
            .await
            .unwrap();
        assert_eq!(
            store
                .find_instance(instance_id)
                .await
                .unwrap()
                .config_revision
                .scope_revision,
            1
        );

        initial.revision = 901;
        store
            .declare_service_scope("svc", &scope, initial)
            .await
            .unwrap();
        assert_eq!(
            store
                .find_instance(instance_id)
                .await
                .unwrap()
                .config_revision
                .scope_revision,
            1
        );

        let mut changed = descriptor(json!({"env": {"MODE": "changed"}}));
        changed.revision = 1;
        store
            .declare_service_scope("svc", &scope, changed.clone())
            .await
            .unwrap();
        assert_eq!(
            store
                .find_instance(instance_id)
                .await
                .unwrap()
                .config_revision
                .scope_revision,
            2
        );

        changed.revision = u64::MAX;
        store
            .declare_service_scope("svc", &scope, changed)
            .await
            .unwrap();
        assert_eq!(
            store
                .find_instance(instance_id)
                .await
                .unwrap()
                .config_revision
                .scope_revision,
            2
        );

        let definition = store.registry.find_definition("svc").await.unwrap();
        assert_eq!(definition.base_revision, 1);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn openapi_runtime_uses_applied_config_until_explicit_restart() {
        let base_url = spawn_openapi_auth_fixture().await;
        let store = MCPStore::setup_with_options(store_options(None)).unwrap();
        let spec = json!({
            "openapi": "3.0.0",
            "info": {"title": "Secure Inventory", "version": "1.0.0"},
            "servers": [{"url": base_url}],
            "paths": {
                "/secure/items": {
                    "post": {
                        "operationId": "createSecureItem",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {"sku": {"type": "string"}},
                                        "required": ["sku"]
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "object"}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        store
            .import_openapi_service_from_spec_with_options(
                "secure",
                "memory://secure",
                spec,
                crate::openapi::OpenApiImportOptions {
                    headers: HashMap::from([(
                        "x-api-key".to_string(),
                        "applied-secret".to_string(),
                    )]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let instance_id = instance_id("secure", store_scope());
        store.restart_service(instance_id).await.unwrap();
        let connected = store.find_instance(instance_id).await.unwrap();
        assert_eq!(
            connected.applied_config_revision,
            Some(connected.config_revision)
        );
        assert_eq!(
            store
                .cache()
                .get_entity("service_instances", &instance_id.to_string())
                .await
                .unwrap()
                .unwrap()["applied_config_revision"],
            serde_json::to_value(connected.config_revision).unwrap()
        );
        let applied_before = store
            .applied_openapi_configs
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap();
        assert_eq!(
            applied_before["headers"]["x-api-key"],
            json!("applied-secret")
        );
        assert!(
            !store
                .call_tool(
                    instance_id,
                    "createSecureItem",
                    json!({"body": {"sku": "sku-1"}}),
                )
                .await
                .unwrap()
                .is_error
        );

        let mut changed: ServerConfig = serde_json::from_value(
            store
                .get_definition_config("secure")
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        changed
            .headers
            .insert("x-api-key".to_string(), "pending-secret".to_string());
        changed.mcpstore.as_mut().unwrap().revision += 1;
        store
            .register_configured_definition("secure", &changed)
            .await
            .unwrap();

        let pending = store.find_instance(instance_id).await.unwrap();
        assert_eq!(
            store
                .state_manager
                .get(instance_id)
                .await
                .unwrap()
                .unwrap()
                .phase,
            crate::state::RuntimePhase::Running
        );
        assert!(pending.restart_required());
        assert_eq!(
            store
                .cache()
                .get_entity("service_instances", &instance_id.to_string())
                .await
                .unwrap()
                .unwrap()["applied_config_revision"],
            serde_json::to_value(pending.applied_config_revision.unwrap()).unwrap()
        );
        assert_eq!(
            store.applied_openapi_configs.read().await.get(&instance_id),
            Some(&applied_before)
        );
        assert!(
            !store
                .call_tool(
                    instance_id,
                    "createSecureItem",
                    json!({"body": {"sku": "sku-1"}}),
                )
                .await
                .unwrap()
                .is_error
        );

        store.restart_service(instance_id).await.unwrap();
        let restarted = store.find_instance(instance_id).await.unwrap();
        assert_eq!(
            restarted.applied_config_revision,
            Some(restarted.config_revision)
        );
        assert_eq!(
            store
                .cache()
                .get_entity("service_instances", &instance_id.to_string())
                .await
                .unwrap()
                .unwrap()["applied_config_revision"],
            serde_json::to_value(restarted.config_revision).unwrap()
        );
        let applied_after = store
            .applied_openapi_configs
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap();
        assert_eq!(
            applied_after["headers"]["x-api-key"],
            json!("pending-secret")
        );
        assert!(
            store
                .call_tool(
                    instance_id,
                    "createSecureItem",
                    json!({"body": {"sku": "sku-1"}}),
                )
                .await
                .unwrap()
                .is_error
        );
    }

    #[tokio::test]
    async fn tool_transform_and_disconnect_are_isolated_between_sibling_instances() {
        let base_url = spawn_openapi_auth_fixture().await;
        let store = MCPStore::setup_with_options(store_options(None)).unwrap();
        let spec = json!({
            "openapi": "3.0.0",
            "info": {"title": "Echo", "version": "1.0.0"},
            "servers": [{"url": base_url}],
            "paths": {
                "/echo": {
                    "post": {
                        "operationId": "echo",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {"text": {"type": "string"}},
                                        "required": ["text"]
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "object"}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        store
            .import_openapi_service_from_spec_with_options(
                "echo-service",
                "memory://echo-service",
                spec,
                crate::openapi::OpenApiImportOptions {
                    headers: HashMap::from([(
                        "x-api-key".to_string(),
                        "applied-secret".to_string(),
                    )]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let mut config: ServerConfig = serde_json::from_value(
            store
                .get_definition_config("echo-service")
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let extension = config.mcpstore.as_mut().unwrap();
        extension.scopes.agents = HashMap::from([
            ("agent-1".to_string(), ScopeDescriptor::default()),
            ("agent-2".to_string(), ScopeDescriptor::default()),
        ]);
        store
            .register_configured_definition("echo-service", &config)
            .await
            .unwrap();

        let agent_1_id = instance_id("echo-service", agent_scope("agent-1"));
        let agent_2_id = instance_id("echo-service", agent_scope("agent-2"));
        store.connect_service(agent_1_id).await.unwrap();
        store.connect_service(agent_2_id).await.unwrap();
        store
            .set_tool_transform(
                agent_1_id,
                "echo",
                ToolTransformPatch::default()
                    .with_display_name("say")
                    .rename_argument("body", "payload"),
            )
            .await
            .unwrap();

        let transformed = store
            .call_tool(agent_1_id, "say", json!({"payload": {"text": "hello"}}))
            .await
            .unwrap();
        assert!(!transformed.is_error);

        let error = store
            .call_tool(agent_2_id, "say", json!({"payload": {"text": "hello"}}))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Tool 'say' not found"));

        let untouched = store
            .call_tool(agent_2_id, "echo", json!({"body": {"text": "hello"}}))
            .await
            .unwrap();
        assert!(!untouched.is_error);

        store.disconnect_service(agent_1_id).await.unwrap();
        let stopped = store.state_manager.get(agent_1_id).await.unwrap().unwrap();
        assert_eq!(stopped.desired, crate::state::DesiredState::Stopped);
        assert_eq!(stopped.phase, crate::state::RuntimePhase::Stopped);
        let sibling = store.state_manager.get(agent_2_id).await.unwrap().unwrap();
        assert_eq!(sibling.desired, crate::state::DesiredState::Running);
        assert_eq!(sibling.phase, crate::state::RuntimePhase::Running);
        assert_eq!(sibling.health, crate::state::HealthState::Healthy);

        let sibling_call = store
            .call_tool(agent_2_id, "echo", json!({"body": {"text": "hello"}}))
            .await
            .unwrap();
        assert!(!sibling_call.is_error, "{sibling_call:?}");
    }

    #[tokio::test]
    async fn removing_store_scope_preserves_agent_scopes_and_definition() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_1_id = instance_id("svc", agent_scope("agent-1"));
        let agent_2_id = instance_id("svc", agent_scope("agent-2"));
        store
            .remove_service_scope("svc", &store_scope())
            .await
            .unwrap();

        assert!(store.find_instance(store_id).await.is_none());
        assert!(store.find_instance(agent_1_id).await.is_some());
        assert!(store.find_instance(agent_2_id).await.is_some());
        let definition = store.registry.find_definition("svc").await.unwrap();
        assert!(definition.scopes.store.is_none());
        assert!(definition.scopes.agents.contains_key("agent-1"));
        assert!(definition.scopes.agents.contains_key("agent-2"));
        let config = store.config_manager.load_or_empty().unwrap();
        let config_scopes = config.mcp_servers["svc"].scopes();
        assert!(config_scopes.store.is_none());
        assert!(config_scopes.agents.contains_key("agent-1"));
        assert!(config_scopes.agents.contains_key("agent-2"));
        assert!(store
            .cache()
            .get_entity("service_instances", &store_id.to_string())
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_entity("service_instances", &agent_1_id.to_string())
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_entity("service_instances", &agent_2_id.to_string())
            .await
            .unwrap()
            .is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn removing_scope_only_removes_its_exact_instance() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_1_scope = agent_scope("agent-1");
        let agent_1_id = instance_id("svc", agent_1_scope.clone());
        let agent_2_id = instance_id("svc", agent_scope("agent-2"));
        install_tool(&store, store_id, tool("store-tool")).await;
        install_tool(&store, agent_1_id, tool("agent-1-tool")).await;
        install_tool(&store, agent_2_id, tool("agent-2-tool")).await;
        store
            .cache_instance_connected(store_id, &[tool("store-tool")])
            .await
            .unwrap();
        store
            .cache_instance_connected(agent_1_id, &[tool("agent-1-tool")])
            .await
            .unwrap();
        store
            .cache_instance_connected(agent_2_id, &[tool("agent-2-tool")])
            .await
            .unwrap();

        let agent_1_session = store
            .create_session(CreateSessionRequest::agent("agent-1-session", "agent-1"))
            .await
            .unwrap();
        store
            .bind_service_to_session(&agent_1_session.session_key, agent_1_id)
            .await
            .unwrap();
        store
            .set_session_tool_visibility(
                &agent_1_session.session_key,
                vec![SessionToolSelection {
                    instance_id: agent_1_id,
                    tool_name: "agent-1-tool".to_string(),
                }],
            )
            .await
            .unwrap();
        store
            .set_context_tool_visibility(agent_1_id, vec!["agent-1-tool".to_string()])
            .await
            .unwrap();
        store
            .set_tool_preference(agent_1_id, "agent-1-tool", "return_direct", json!(true))
            .await
            .unwrap();
        store
            .set_tool_transform(
                agent_1_id,
                "agent-1-tool",
                ToolTransformPatch::default().with_display_name("agent-1-renamed"),
            )
            .await
            .unwrap();

        let agent_2_session = store
            .create_session(CreateSessionRequest::agent("agent-2-session", "agent-2"))
            .await
            .unwrap();
        store
            .bind_service_to_session(&agent_2_session.session_key, agent_2_id)
            .await
            .unwrap();
        store
            .set_session_tool_visibility(
                &agent_2_session.session_key,
                vec![SessionToolSelection {
                    instance_id: agent_2_id,
                    tool_name: "agent-2-tool".to_string(),
                }],
            )
            .await
            .unwrap();
        store
            .set_context_tool_visibility(agent_2_id, vec!["agent-2-tool".to_string()])
            .await
            .unwrap();
        store
            .set_tool_preference(agent_2_id, "agent-2-tool", "return_direct", json!(true))
            .await
            .unwrap();
        store
            .set_tool_transform(
                agent_2_id,
                "agent-2-tool",
                ToolTransformPatch::default().with_display_name("agent-2-renamed"),
            )
            .await
            .unwrap();

        store
            .remove_service_scope("svc", &agent_1_scope)
            .await
            .unwrap();

        assert!(store.find_instance(agent_1_id).await.is_none());
        assert!(store.find_instance(store_id).await.is_some());
        assert!(store.find_instance(agent_2_id).await.is_some());
        assert!(store
            .cache()
            .get_entity("service_instances", &agent_1_id.to_string())
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_entity("tools", &format!("{agent_1_id}:agent-1-tool"))
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_entity("tools", &format!("{store_id}:store-tool"))
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_entity("tools", &format!("{agent_2_id}:agent-2-tool"))
            .await
            .unwrap()
            .is_some());
        assert!(store
            .list_session_services(&agent_1_session.session_key)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .list_session_tools(&agent_1_session.session_key)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .list_session_services(&agent_2_session.session_key)
                .await
                .unwrap()[0]
                .instance_id,
            agent_2_id
        );
        assert_eq!(
            store
                .list_session_tools(&agent_2_session.session_key)
                .await
                .unwrap()[0]
                .instance_id,
            agent_2_id
        );
        for (state_type, state_key) in [
            (
                "context_tool_visibility",
                format!("agent:agent-1:{agent_1_id}"),
            ),
            (
                "tool_preferences",
                format!("agent:agent-1:{agent_1_id}:agent-1-tool"),
            ),
            ("tool_transforms", format!("{agent_1_id}:agent-1-tool")),
        ] {
            assert!(store
                .cache()
                .get_state(state_type, &state_key)
                .await
                .unwrap()
                .is_none());
        }
        for (state_type, state_key) in [
            (
                "context_tool_visibility",
                format!("agent:agent-2:{agent_2_id}"),
            ),
            (
                "tool_preferences",
                format!("agent:agent-2:{agent_2_id}:agent-2-tool"),
            ),
            ("tool_transforms", format!("{agent_2_id}:agent-2-tool")),
        ] {
            assert!(store
                .cache()
                .get_state(state_type, &state_key)
                .await
                .unwrap()
                .is_some());
        }

        let definition = store.registry.find_definition("svc").await.unwrap();
        assert!(definition.scopes.store.is_some());
        assert!(!definition.scopes.agents.contains_key("agent-1"));
        assert!(definition.scopes.agents.contains_key("agent-2"));
        let config = store.config_manager.load_or_empty().unwrap();
        let config_scopes = config.mcp_servers["svc"].scopes();
        assert!(config_scopes.store.is_some());
        assert!(!config_scopes.agents.contains_key("agent-1"));
        assert!(config_scopes.agents.contains_key("agent-2"));
        let cached_definition: ServiceDefinitionEntity = serde_json::from_value(
            store
                .cache()
                .get_entity("service_definitions", "svc")
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(cached_definition.scopes, definition.scopes);

        let db = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(format!("scope-remove-db-{}", uuid::Uuid::new_v4())),
        })
        .unwrap();
        copy_cache_snapshot(&store, &db).await;
        db.load_from_db().await.unwrap();
        let db_definition = db.registry.find_definition("svc").await.unwrap();
        assert!(db_definition.scopes.store.is_some());
        assert!(!db_definition.scopes.agents.contains_key("agent-1"));
        assert!(db_definition.scopes.agents.contains_key("agent-2"));
        assert!(db.find_instance(agent_1_id).await.is_none());
        assert!(db.find_instance(store_id).await.is_some());
        assert!(db.find_instance(agent_2_id).await.is_some());

        let restored_id = store
            .declare_service_scope("svc", &agent_1_scope, ScopeDescriptor::default())
            .await
            .unwrap();
        assert_eq!(restored_id, agent_1_id);
        assert!(store
            .list_session_services(&agent_1_session.session_key)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .list_session_tools(&agent_1_session.session_key)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .cache()
            .get_state(
                "context_tool_visibility",
                &format!("agent:agent-1:{agent_1_id}"),
            )
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_state(
                "tool_preferences",
                &format!("agent:agent-1:{agent_1_id}:agent-1-tool"),
            )
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_state("tool_transforms", &format!("{agent_1_id}:agent-1-tool"))
            .await
            .unwrap()
            .is_none());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn reset_scope_updates_all_definition_projections() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("alpha", native_config(scopes.clone()))
            .await
            .unwrap();
        store
            .add_service("beta", native_config(scopes))
            .await
            .unwrap();

        let agent_1_scope = agent_scope("agent-1");
        store.reset_scope(&agent_1_scope).await.unwrap();

        let saved = store.config_manager.load_or_empty().unwrap();
        for service_name in ["alpha", "beta"] {
            let config_scopes = saved.mcp_servers[service_name].scopes();
            assert!(config_scopes.store.is_some());
            assert!(!config_scopes.agents.contains_key("agent-1"));
            assert!(config_scopes.agents.contains_key("agent-2"));

            let definition = store.registry.find_definition(service_name).await.unwrap();
            assert_eq!(definition.scopes, config_scopes);
            let cached: ServiceDefinitionEntity = serde_json::from_value(
                store
                    .cache()
                    .get_entity("service_definitions", service_name)
                    .await
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(cached.scopes, config_scopes);

            assert!(store
                .find_instance(instance_id(service_name, agent_1_scope.clone()))
                .await
                .is_none());
            assert!(store
                .find_instance(instance_id(service_name, store_scope()))
                .await
                .is_some());
            assert!(store
                .find_instance(instance_id(service_name, agent_scope("agent-2")))
                .await
                .is_some());
        }

        let db = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(format!("scope-reset-db-{}", uuid::Uuid::new_v4())),
        })
        .unwrap();
        copy_cache_snapshot(&store, &db).await;
        db.load_from_db().await.unwrap();
        assert!(db
            .list_scope_instances(&agent_1_scope)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            db.list_scope_instances(&store_scope()).await.unwrap().len(),
            2
        );
        assert_eq!(
            db.list_scope_instances(&agent_scope("agent-2"))
                .await
                .unwrap()
                .len(),
            2
        );
        for service_name in ["alpha", "beta"] {
            let definition = db.registry.find_definition(service_name).await.unwrap();
            assert!(definition.scopes.store.is_some());
            assert!(!definition.scopes.agents.contains_key("agent-1"));
            assert!(definition.scopes.agents.contains_key("agent-2"));
        }

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn tool_transforms_are_isolated_by_instance_id() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: None,
            agents: HashMap::from([
                ("agent-1".to_string(), ScopeDescriptor::default()),
                ("agent-2".to_string(), ScopeDescriptor::default()),
            ]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let agent_1_id = instance_id("svc", agent_scope("agent-1"));
        let agent_2_id = instance_id("svc", agent_scope("agent-2"));
        install_tool(&store, agent_1_id, tool("echo")).await;
        install_tool(&store, agent_2_id, tool("echo")).await;

        let rule = store
            .set_tool_transform(
                agent_1_id,
                "echo",
                ToolTransformPatch::default()
                    .with_display_name("say")
                    .rename_argument("text", "message")
                    .hide_argument("debug", false),
            )
            .await
            .unwrap();
        assert_eq!(rule.instance_id, agent_1_id);
        assert_eq!(rule.scope, agent_scope("agent-1"));
        assert_eq!(rule.tool_name, "echo");

        assert!(store
            .get_tool_transform(agent_1_id, "echo")
            .await
            .unwrap()
            .is_some());
        assert!(store
            .get_tool_transform(agent_2_id, "echo")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_state("tool_transforms", &format!("{agent_1_id}:echo"))
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_state("tool_transforms", &format!("{agent_2_id}:echo"))
            .await
            .unwrap()
            .is_none());

        let (_, resolved_name, transformed_args) = store
            .resolve_transformed_tool_call(agent_1_id, "say", json!({"message": "hello"}))
            .await
            .unwrap();
        assert_eq!(resolved_name, "echo");
        assert_eq!(transformed_args, json!({"text": "hello", "debug": false}));
        let (_, resolved_name, untouched_args) = store
            .resolve_transformed_tool_call(agent_2_id, "echo", json!({"text": "hello"}))
            .await
            .unwrap();
        assert_eq!(resolved_name, "echo");
        assert_eq!(untouched_args, json!({"text": "hello"}));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn instance_owned_policy_apis_derive_scope_from_instance_id() {
        let path = temp_config_path();
        let scopes = ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::from([("agent-1".to_string(), ScopeDescriptor::default())]),
        };
        let store = MCPStore::setup_with_options(store_options(Some(path.clone()))).unwrap();
        store
            .add_service("svc", native_config(scopes))
            .await
            .unwrap();

        let store_id = instance_id("svc", store_scope());
        let agent_id = instance_id("svc", agent_scope("agent-1"));
        install_tool(&store, store_id, tool("echo")).await;
        install_tool(&store, agent_id, tool("echo")).await;

        let store_visibility = store
            .set_context_tool_visibility(store_id, vec!["echo".to_string()])
            .await
            .unwrap();
        let agent_visibility = store
            .set_context_tool_visibility(agent_id, Vec::new())
            .await
            .unwrap();
        assert_eq!(store_visibility.context_key, "store");
        assert_eq!(store_visibility.scope, store_scope());
        assert_eq!(agent_visibility.context_key, "agent:agent-1");
        assert_eq!(agent_visibility.scope, agent_scope("agent-1"));
        assert_eq!(
            store
                .list_tool_entries_for_instance_with_filter(
                    store_id,
                    ToolVisibilityFilter::Available,
                )
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(store
            .list_tool_entries_for_instance_with_filter(agent_id, ToolVisibilityFilter::Available,)
            .await
            .unwrap()
            .is_empty());

        store
            .set_tool_preference(store_id, "echo", "return_direct", json!(true))
            .await
            .unwrap();
        store
            .set_tool_preference(agent_id, "echo", "return_direct", json!(false))
            .await
            .unwrap();
        assert_eq!(
            store
                .get_tool_preference(store_id, "echo", "return_direct")
                .await
                .unwrap(),
            Some(json!(true))
        );
        assert_eq!(
            store
                .get_tool_preference(agent_id, "echo", "return_direct")
                .await
                .unwrap(),
            Some(json!(false))
        );

        std::fs::remove_file(path).ok();
    }
}

#[tokio::test]
async fn first_oauth_connection_returns_auth_required_without_network_retry() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            observed.fetch_add(1, Ordering::SeqCst);
            let body = br#"{\"error\":\"unexpected network request\"}"#;
            let response = format!(
                "HTTP/1.1 500 Internal Server Error\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.write_all(body).await;
        }
    });

    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-first-oauth-auth-required".to_string()),
    })
    .unwrap();
    let mut config = oauth_http_config();
    config.url = Some(format!("http://{addr}/mcp"));
    store.add_service("protected", config).await.unwrap();
    let instance_id = store_instance_id("protected");

    let error = store.connect_service(instance_id).await.unwrap_err();
    let StoreError::Transport(crate::transport::TransportError::AuthRequired(required)) = error
    else {
        panic!("expected structured auth_required transport error");
    };
    assert_eq!(required.instance_id, instance_id);
    assert_eq!(required.flow, crate::auth::AuthFlow::AuthorizationCode);
    assert_eq!(required.scopes, vec!["tools.read"]);
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(requests.load(Ordering::SeqCst), 0);

    let state = store.state_manager.get(instance_id).await.unwrap().unwrap();
    assert_eq!(state.phase, crate::state::RuntimePhase::Stopped);
    assert_eq!(state.recovery, crate::state::RecoveryState::Idle);
    assert_eq!(state.failure, None);
    assert_eq!(
        store.auth_status(instance_id).await,
        crate::auth::AuthStatus::Unauthenticated
    );

    server.abort();
    std::fs::remove_file(path).ok();
}

#[cfg(test)]
mod event_reactor_facade {
    use super::*;
    use crate::event_reactor::{ReactionOutcome, ReactorConfig, Rule};
    use crate::store::CacheStorage;
    use tokio::sync::Notify;
    use tokio::time::Duration;

    /// MCPStore facade: setup reactor → register rule → start → write → trigger.
    #[tokio::test]
    async fn facade_reactor_end_to_end_memory() {
        let store = MCPStore::setup_with_options(StoreOptions {
            backend: Some(CacheStorage::Memory),
            ..StoreOptions::default()
        })
        .unwrap();

        let config = ReactorConfig {
            subscriber_id: "facade-test-sub".into(),
            owner_id: "facade-test-owner".into(),
            namespace: "mcpstore".into(),
            watch_collections: vec!["mcpstore:event:facade.test".into()],
            max_causation_depth: 16,
        };

        store.setup_event_reactor(config).await.unwrap();
        assert!(store.has_reactor().await);

        let notify = Arc::new(Notify::new());
        let nc = notify.clone();
        store
            .register_rule(Rule::new(
                "facade.rule.v1",
                |ctx| Box::pin(async move { ctx.collection == "mcpstore:event:facade.test" }),
                move |_ctx| {
                    let nc = nc.clone();
                    Box::pin(async move {
                        nc.notify_one();
                        ReactionOutcome::Ok
                    })
                },
            ))
            .await
            .unwrap();

        store.start_reactor().await.unwrap();

        // Give subscription a moment to establish.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Write a value to the watched collection via the cache layer — the
        // EventReactor's independent backend will see it via ChangeFeed.
        store
            .cache
            .put_event(
                "facade.test",
                "evt-1",
                serde_json::json!({"source": "facade-test"}),
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(5), notify.notified())
            .await
            .expect("facade reaction did not fire");

        store.stop_reactor().await;
    }

    /// Calling facade methods without setup_event_reactor should error gracefully.
    #[tokio::test]
    async fn facade_reactor_not_initialized_errors() {
        let store = MCPStore::setup_with_options(StoreOptions {
            backend: Some(CacheStorage::Memory),
            ..StoreOptions::default()
        })
        .unwrap();

        assert!(!store.has_reactor().await);

        let result = store.start_reactor().await;
        assert!(result.is_err());

        store.stop_reactor().await; // no-op, should not panic
    }
}

#[cfg(test)]
mod control_reactor_tests {
    use super::*;
    use crate::event_reactor::ReactorConfig;
    use crate::store::CacheStorage;
    use tokio::time::Duration;

    /// Full pipeline: setup reactor → register control_request_rule → start →
    /// queue a pending control request → verify it's auto-processed to completed.
    #[tokio::test]
    async fn control_reactor_auto_processes_pending_request() {
        let path = temp_config_path();
        let store = std::sync::Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: Some(path.clone()),
                source_mode: SourceMode::Local,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some("test-control-reactor".to_string()),
            })
            .unwrap(),
        );

        let collection = store.cache().event_collection(CONTROL_REQUEST_EVENT_TYPE);

        let config = ReactorConfig {
            subscriber_id: "control-test-sub".into(),
            owner_id: "control-test-owner".into(),
            namespace: "test-control-reactor".into(),
            watch_collections: vec![collection.clone()],
            max_causation_depth: 16,
        };
        store.setup_event_reactor(config).await.unwrap();
        let rule = store.control_request_rule();
        store.register_rule(rule).await.unwrap();
        store.start_reactor().await.unwrap();

        // Give subscription a moment to establish.
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Queue a pending control request (ServiceAddRequested).
        store
            .cache()
            .put_event(
                CONTROL_REQUEST_EVENT_TYPE,
                "reactor-add",
                serde_json::json!({
                    "id": "reactor-add",
                    "type": "ServiceAddRequested",
                    "payload": {
                        "service_name": "reactor-svc",
                        "config": stdio_config(),
                    },
                    "source": "onlydb",
                    "created_at": 222,
                    "dedup_key": "ServiceAddRequested:reactor-svc",
                    "trace_id": "reactor-add",
                    "status": "queued",
                }),
            )
            .await
            .unwrap();

        // Poll for completion (the reactor processes asynchronously).
        let mut completed = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Some(evt) = store
                .cache()
                .get_event(CONTROL_REQUEST_EVENT_TYPE, "reactor-add")
                .await
                .unwrap()
            {
                if evt["status"] == serde_json::json!("applied") {
                    completed = true;
                    break;
                }
            }
        }
        assert!(
            completed,
            "control request was not auto-processed by reactor"
        );

        // Verify the service was actually added.
        assert!(store
            .cache()
            .get_entity("service_definitions", "reactor-svc")
            .await
            .unwrap()
            .is_some());

        store.stop_reactor().await;
        std::fs::remove_file(path).ok();
    }

    /// Verify the Rule's `when` predicate rejects non-pending requests (recursion guard).
    #[tokio::test]
    async fn control_reactor_skips_non_pending_requests() {
        let path = temp_config_path();
        let store = std::sync::Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: Some(path.clone()),
                source_mode: SourceMode::Local,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some("test-control-reactor-skip".to_string()),
            })
            .unwrap(),
        );

        // Insert a request that's already completed.
        store
            .cache()
            .put_event(
                CONTROL_REQUEST_EVENT_TYPE,
                "already-done",
                serde_json::json!({
                    "id": "already-done",
                    "type": "ServiceAddRequested",
                    "payload": {
                        "service_name": "should-not-run",
                        "config": stdio_config(),
                    },
                    "source": "onlydb",
                    "created_at": 333,
                    "dedup_key": "ServiceAddRequested:should-not-run",
                    "trace_id": "already-done",
                    "status": "applied",
                    "applied_at": 333,
                }),
            )
            .await
            .unwrap();

        let collection = store.cache().event_collection(CONTROL_REQUEST_EVENT_TYPE);

        let config = ReactorConfig {
            subscriber_id: "control-skip-sub".into(),
            owner_id: "control-skip-owner".into(),
            namespace: "test-control-reactor-skip".into(),
            watch_collections: vec![collection],
            max_causation_depth: 16,
        };
        store.setup_event_reactor(config).await.unwrap();
        let rule = store.control_request_rule();
        store.register_rule(rule).await.unwrap();
        store.start_reactor().await.unwrap();

        // Wait enough time for any (incorrect) reaction to fire.
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Verify no new "should-not-run" service was created.
        let svc = store
            .cache()
            .get_entity("service_definitions", "should-not-run")
            .await
            .unwrap();
        assert!(
            svc.is_none(),
            "rule should have skipped non-pending request, but service was created"
        );

        store.stop_reactor().await;
        std::fs::remove_file(path).ok();
    }
}
