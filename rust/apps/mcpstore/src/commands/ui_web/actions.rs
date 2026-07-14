use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use maud::html;
use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use mcpstore::{CacheStorage, InstanceId, MCPStore, ScopeRef};
use std::{collections::HashMap, sync::Arc};

use super::{
    components::{error_markup, modal_frame, modal_notice},
    layout::layout,
    utils::{parse_kv_lines, pretty_json, trim_optional, url_component},
};

pub(super) async fn action_connect(
    State(store): State<Arc<MCPStore>>,
    Path(instance_id): Path<InstanceId>,
) -> impl IntoResponse {
    match store.connect_service(instance_id).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(e) => Html(layout("mcpstore - Error", error_markup(&e.to_string())).into_string())
            .into_response(),
    }
}

pub(super) async fn action_disconnect(
    State(store): State<Arc<MCPStore>>,
    Path(instance_id): Path<InstanceId>,
) -> impl IntoResponse {
    match store.disconnect_service(instance_id).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(e) => Html(layout("mcpstore - Error", error_markup(&e.to_string())).into_string())
            .into_response(),
    }
}

pub(super) async fn action_restart(
    State(store): State<Arc<MCPStore>>,
    Path(instance_id): Path<InstanceId>,
) -> impl IntoResponse {
    match store.restart_service(instance_id).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(e) => Html(layout("mcpstore - Error", error_markup(&e.to_string())).into_string())
            .into_response(),
    }
}

pub(super) async fn action_remove(
    State(store): State<Arc<MCPStore>>,
    Path(instance_id): Path<InstanceId>,
) -> impl IntoResponse {
    let Some(instance) = store.find_instance(instance_id).await else {
        return Html(
            layout(
                "mcpstore - Error",
                error_markup("Service instance not found"),
            )
            .into_string(),
        )
        .into_response();
    };
    match store
        .remove_service_scope(&instance.service_name, &instance.scope)
        .await
    {
        Ok(_) => Redirect::to("/").into_response(),
        Err(e) => Html(layout("mcpstore - Error", error_markup(&e.to_string())).into_string())
            .into_response(),
    }
}

pub(super) async fn action_switch_cache_storage(
    State(store): State<Arc<MCPStore>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let target = params.get("target").map(String::as_str).unwrap_or("");
    let cache_storage = match target {
        "memory" => CacheStorage::Memory,
        "redis" => CacheStorage::Redis,
        "openkeyv_memory" => CacheStorage::OpenKeyvMemory,
        "openkeyv_redis" => CacheStorage::OpenKeyvRedis,
        _ => {
            return Html(
                layout(
                    "mcpstore - Error",
                    error_markup(
                        "Target cache storage must be memory, redis, openkeyv_memory, or openkeyv_redis",
                    ),
                )
                .into_string(),
            )
            .into_response();
        }
    };
    match store.switch_cache_storage(cache_storage, None, None).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(e) => Html(layout("mcpstore - Error", error_markup(&e.to_string())).into_string())
            .into_response(),
    }
}

pub(super) async fn modal_switch_cache_storage(
    State(store): State<Arc<MCPStore>>,
) -> impl IntoResponse {
    let cache_storage = store.current_cache_storage().await;
    let current_label = match cache_storage {
        CacheStorage::Memory => "memory",
        CacheStorage::Redis => "redis",
        CacheStorage::OpenKeyvMemory => "openkeyv_memory",
        CacheStorage::OpenKeyvRedis => "openkeyv_redis",
    };
    let content = html! {
        dialog open {
            article {
                header.modal-header {
                    div {
                        h3 { "Data hot migration" }
                        p.hint { "Current cache storage: " code { (current_label) } }
                    }
                    button.button.button-ghost type="button" onclick="closeModal()" { "Close" }
                }
                form.modal-form method="get" action="/action/switch-cache-storage" {
                    div.field {
                        label for="field-target" { "Target cache storage" }
                        select id="field-target" name="target" {
                            option value="memory" selected[cache_storage == CacheStorage::Memory] { "memory" }
                            option value="redis" selected[cache_storage == CacheStorage::Redis] { "redis" }
                            option value="openkeyv_memory" selected[cache_storage == CacheStorage::OpenKeyvMemory] { "openkeyv_memory" }
                            option value="openkeyv_redis" selected[cache_storage == CacheStorage::OpenKeyvRedis] { "openkeyv_redis" }
                        }
                    }
                    footer {
                        button.button.button-primary type="submit" { "Switch" }
                        button.button.button-ghost type="button" onclick="closeModal()" { "Cancel" }
                    }
                }
            }
        }
    };
    Html(content.into_string())
}

pub(super) async fn modal_call_tool_form(
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> impl IntoResponse {
    let instance_segment = instance_id.to_string();
    let tool_segment = url_component(&tool_name);
    let body = html! {
        form.modal-form method="get" action=(format!("/modal/call-tool/{instance_segment}/{tool_segment}/exec")) {
            div.field {
                label for="field-args" { "Args JSON" }
                textarea id="field-args" name="args" placeholder="{}" { "{}" }
                p.hint { "Args must be a JSON object." }
            }
            footer {
                button.button.button-primary type="submit" { "Run" }
                button.button.button-ghost type="button" onclick="closeModal()" { "Cancel" }
            }
        }
    };
    Html(
        modal_frame(
            &format!("Run tool: {tool_name}"),
            Some(&format!("Instance: {instance_id}")),
            body,
            None,
        )
        .into_string(),
    )
}

pub(super) async fn modal_call_tool_exec(
    State(store): State<Arc<MCPStore>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let args_str = params
        .get("args")
        .cloned()
        .unwrap_or_else(|| "{}".to_string());
    let args = match serde_json::from_str::<serde_json::Value>(&args_str) {
        Ok(v) if v.is_object() => v,
        Ok(_) => {
            return Html(
                modal_notice("Param error", "Tool args must be a JSON object").into_string(),
            )
            .into_response();
        }
        Err(e) => {
            return Html(
                modal_notice("Param error", &format!("JSON parse failed: {e}")).into_string(),
            )
            .into_response();
        }
    };

    match store.call_tool(instance_id, &tool_name, args).await {
        Ok(result) => {
            let result_json = pretty_json(&result);
            let body = html! {
                pre.result-block class=[if result.is_error { Some("is-error") } else { None }] {
                    code { (result_json) }
                }
            };
            Html(
                modal_frame(
                    "Result",
                    Some(&format!("{instance_id}.{tool_name}")),
                    body,
                    Some(html! { button.button.button-primary type="button" onclick="closeModal()" { "Close" } }),
                )
                .into_string(),
            )
            .into_response()
        }
        Err(e) => {
            Html(modal_notice("Execution failed", &e.to_string()).into_string()).into_response()
        }
    }
}

pub(super) async fn modal_tool_detail(
    State(store): State<Arc<MCPStore>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> impl IntoResponse {
    let svc = match store.find_instance(instance_id).await {
        Some(s) => s,
        None => return Html(modal_notice("Error", "Service not found").into_string()),
    };
    let tool = match svc.tools.iter().find(|t| t.name == tool_name) {
        Some(t) => t,
        None => return Html(modal_notice("Error", "Tool not found").into_string()),
    };

    let schema_json = pretty_json(&tool.input_schema);
    let body = html! {
        div.modal-stack {
            @if !tool.description.is_empty() {
                div.detail-block {
                    span.detail-label { "Description" }
                    p { (tool.description) }
                }
            }
            div.detail-block {
                span.detail-label { "Param Schema" }
                pre.result-block { code { (schema_json) } }
            }
        }
    };
    Html(
        modal_frame(
            &tool.name,
            Some(&format!("{}.{tool_name}", svc.service_name)),
            body,
            Some(html! { button.button.button-primary type="button" onclick="closeModal()" { "Close" } }),
        )
        .into_string(),
    )
}

pub(super) async fn action_add_exec(
    State(store): State<Arc<MCPStore>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(name) = trim_optional(params.get("name")) else {
        return form_error("Name cannot be empty");
    };
    let Some(command_or_url) = trim_optional(params.get("command_or_url")) else {
        return form_error("Command or URL cannot be empty");
    };
    let transport = params
        .get("transport")
        .map(String::as_str)
        .unwrap_or("stdio");
    let resolved_transport = match transport {
        "stdio" => "stdio",
        "http" | "streamable-http" => "streamable-http",
        "sse" => "sse",
        _ => return form_error("Transport must be stdio, streamable-http, or sse"),
    };
    let scope = params.get("scope").map(String::as_str).unwrap_or("store");
    let agent = trim_optional(params.get("agent"));
    if scope == "agent" && agent.is_none() {
        return form_error("Agent ID is required when using Agent scope");
    }
    if scope != "store" && scope != "agent" {
        return form_error("Scope must be Store or Agent");
    }

    let env = match parse_kv_lines(
        params.get("env").map(String::as_str).unwrap_or(""),
        "env vars",
    ) {
        Ok(env) => env,
        Err(e) => return form_error(&e),
    };
    let headers = match parse_kv_lines(
        params.get("headers").map(String::as_str).unwrap_or(""),
        "headers",
    ) {
        Ok(headers) => headers,
        Err(e) => return form_error(&e),
    };

    let is_stdio = resolved_transport == "stdio";
    if !(is_stdio
        || command_or_url.starts_with("http://")
        || command_or_url.starts_with("https://"))
    {
        return form_error("Remote transport requires http:// or https:// URL");
    }

    let (command, args) = if is_stdio {
        let mut parts = command_or_url
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return form_error("stdio command cannot be empty");
        }
        let cmd = parts.remove(0);
        (Some(cmd), parts)
    } else {
        (None, Vec::new())
    };

    let mut config = ServerConfig {
        url: if is_stdio {
            None
        } else {
            Some(command_or_url.clone())
        },
        command,
        args,
        env,
        headers,
        transport: Some(resolved_transport.to_string()),
        working_dir: trim_optional(params.get("working_dir")),
        description: trim_optional(params.get("description")),
        mcpstore: None,
        extra: Default::default(),
    };

    let target_scope = match agent {
        Some(agent_id) => ScopeRef::Agent { agent_id },
        None => ScopeRef::Store,
    };
    let definitions = match store.show_config_entry().await {
        Ok(config) => config.mcp_servers,
        Err(error) => {
            return Html(
                layout("mcpstore - Error", error_markup(&error.to_string())).into_string(),
            )
            .into_response();
        }
    };

    let result = if definitions.contains_key(&name) {
        let mut override_config = match serde_json::to_value(&config) {
            Ok(serde_json::Value::Object(config)) => config,
            Ok(_) => return form_error("Service config must be a JSON object"),
            Err(error) => return form_error(&error.to_string()),
        };
        override_config.remove("_mcpstore");
        store
            .declare_service_scope(
                &name,
                &target_scope,
                ScopeDescriptor {
                    config: override_config,
                    lifecycle: None,
                    revision: 0,
                },
            )
            .await
            .map(|_| ())
    } else {
        let mut scopes = ScopeDeclarations::default();
        match &target_scope {
            ScopeRef::Store => scopes.store = Some(ScopeDescriptor::default()),
            ScopeRef::Agent { agent_id } => {
                scopes
                    .agents
                    .insert(agent_id.clone(), ScopeDescriptor::default());
            }
        }
        config.mcpstore = Some(McpStoreExtension {
            scopes,
            ..McpStoreExtension::default()
        });
        store.add_service(&name, config).await
    };

    if let Err(error) = result {
        return Html(layout("mcpstore - Error", error_markup(&error.to_string())).into_string())
            .into_response();
    }

    Redirect::to("/").into_response()
}

fn form_error(msg: &str) -> axum::response::Response {
    Html(layout("mcpstore - Error", error_markup(msg)).into_string()).into_response()
}
