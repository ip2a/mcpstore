use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use maud::html;
use mcpstore::registry::ConnectionStatus;
use mcpstore::{CacheStorage, MCPStore};
use std::{collections::HashMap, sync::Arc};

use super::{
    components::{
        config_block, error_markup, render_service_row, render_service_table_header,
        render_tool_card, status_badge,
    },
    layout::layout,
    utils::{format_added_time, url_component},
};

pub(super) async fn page_home(
    State(store): State<Arc<MCPStore>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let agent_filter = params.get("agent").cloned().unwrap_or_default();
    let cache_storage_label = match store.current_cache_storage().await {
        CacheStorage::Memory => "memory",
        CacheStorage::Redis => "redis",
        CacheStorage::OpenKeyvMemory => "openkeyv_memory",
        CacheStorage::OpenKeyvRedis => "openkeyv_redis",
    };
    let source_label = if store.is_db_source() { "db" } else { "local" };

    let cfg = store.config_manager().load_or_default();
    let mut agents: Vec<(String, Vec<String>)> = cfg.agents.into_iter().collect();
    agents.sort_by(|a, b| a.0.cmp(&b.0));

    let agent_map: HashMap<String, String> = {
        let mut map = HashMap::new();
        for (agent_id, service_names) in &agents {
            for name in service_names {
                map.insert(name.clone(), agent_id.clone());
            }
        }
        map
    };

    let mut services = if agent_filter.is_empty() || agent_filter == "store" {
        store.list_services().await
    } else {
        let names = match store.list_agent_service_names(&agent_filter).await {
            Ok(names) => names,
            Err(e) => {
                return Html(
                    layout("mcpstore - Error", error_markup(&e.to_string())).into_string(),
                )
                .into_response();
            }
        };
        let mut scoped = Vec::new();
        for name in names {
            if let Some(svc) = store.find_service(&name).await {
                scoped.push(svc);
            }
        }
        scoped
    };
    services.sort_by(|a, b| a.name.cmp(&b.name));

    let total = services.len();
    let connected = services
        .iter()
        .filter(|s| s.status == ConnectionStatus::Connected)
        .count();
    let disconnected = services
        .iter()
        .filter(|s| s.status == ConnectionStatus::Disconnected)
        .count();
    let connecting = services
        .iter()
        .filter(|s| s.status == ConnectionStatus::Connecting)
        .count();
    let error = services
        .iter()
        .filter(|s| s.status == ConnectionStatus::Error)
        .count();
    let total_tools: usize = services.iter().map(|s| s.tools.len()).sum();

    let mut transport_counts: HashMap<String, usize> = HashMap::new();
    for service in &services {
        *transport_counts
            .entry(service.transport.clone())
            .or_insert(0) += 1;
    }
    let mut transport_counts: Vec<(String, usize)> = transport_counts.into_iter().collect();
    transport_counts.sort_by(|a, b| a.0.cmp(&b.0));

    let content = html! {
        section.page-heading {
            div {
                p.eyebrow { "MCPStore" }
                h1 { "Service Console" }
            }
            div.heading-actions {
                a.button.button-primary href="/add" { "Add Service" }
                a.button.button-ghost href="/" { "Refresh" }
                button.button type="button" data-modal="/modal/switch-cache-storage" { "Data hot migration" }
            }
        }

        section.dashboard-grid aria-label="Service Overview" {
            div.metric-card.metric-strong {
                span.metric-value { (total) }
                span.metric-label { "Services" }
            }
            div.metric-card {
                span.metric-value { (total_tools) }
                span.metric-label { "Tools" }
            }
            div.metric-card {
                span.metric-value { (connected) }
                span.metric-label { "Connected" }
            }
            div.metric-card {
                span.metric-value { (disconnected) }
                span.metric-label { "Disconnected" }
            }
            @if connecting > 0 {
                div.metric-card.metric-warning {
                    span.metric-value { (connecting) }
                    span.metric-label { "Connecting" }
                }
            }
            @if error > 0 {
                div.metric-card.metric-danger {
                    span.metric-value { (error) }
                    span.metric-label { "Error" }
                }
            }
        }

        section.control-strip {
            div.control-group {
                span.control-label { "Data source" }
                span.meta-pill { (source_label) }
                span.meta-pill { (cache_storage_label) }
            }
            @if !transport_counts.is_empty() {
                div.control-group {
                    span.control-label { "Transport" }
                    @for (transport, count) in &transport_counts {
                        span.meta-pill { (transport) " · " (count) }
                    }
                }
            }
            @if !agents.is_empty() {
                div.control-group.control-group-wide {
                    span.control-label { "Agent" }
                    nav.filter-chips aria-label="Agent filter" {
                        a class=(if agent_filter.is_empty() || agent_filter == "store" { "chip is-active" } else { "chip" }) href="/" { "Store" }
                        @for (agent_id, _) in &agents {
                            @let agent_href = format!("/?agent={}", url_component(agent_id));
                            a class=(if agent_filter == *agent_id { "chip is-active" } else { "chip" }) href=(agent_href) { (agent_id) }
                        }
                    }
                }
            }
        }

        section.list-section {
            div.section-heading {
                h2 { "Service List" }
                span.section-count { (services.len()) " items" }
            }
            @if services.is_empty() {
                div.empty-state {
                    h2 { "No services" }
                    p { "No MCP services available in the current view." }
                    a.button.button-primary href="/add" { "Add Service" }
                }
            } @else {
                div.service-table {
                    (render_service_table_header())
                    @for svc in &services {
                        (render_service_row(svc, agent_map.get(&svc.name).map(|s| s.as_str()).unwrap_or("store")))
                    }
                }
            }
        }
    };

    Html(layout("mcpstore", content).into_string()).into_response()
}

pub(super) async fn page_service(
    State(store): State<Arc<MCPStore>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let svc = match store.find_service(&name).await {
        Some(s) => s,
        None => {
            return Html(
                layout("mcpstore - Error", error_markup("Service not found")).into_string(),
            )
            .into_response();
        }
    };
    let service_segment = url_component(&svc.name);
    let added = format_added_time(svc.added_time);
    let endpoint = svc.url.as_deref().or(svc.command.as_deref()).unwrap_or("-");

    let content = html! {
        section.page-heading {
            div {
                p.eyebrow { (svc.transport) }
                h1 { (svc.name) }
                div.service-meta {
                    (status_badge(svc.status))
                    span.meta-pill { "tools · " (svc.tools.len()) }
                    span.meta-pill { "added · " (added) }
                }
            }
            div.heading-actions {
                a.button.button-ghost href="/" { "Back" }
                @if svc.status == ConnectionStatus::Connected {
                    a.button href=(format!("/action/disconnect/{service_segment}")) { "Disconnect" }
                } @else {
                    a.button.button-primary href=(format!("/action/connect/{service_segment}")) { "Connect" }
                }
                a.button href=(format!("/action/restart/{service_segment}")) { "Restart" }
                a.button.button-danger href=(format!("/action/remove/{service_segment}")) onclick="return confirm('Confirm delete service?')" { "Delete" }
            }
        }

        section.detail-grid {
            div.detail-item {
                span.detail-label { "Name" }
                code { (svc.name) }
            }
            div.detail-item {
                span.detail-label { "Endpoint" }
                code { (endpoint) }
            }
            div.detail-item {
                span.detail-label { "Agent" }
                code { (svc.agent_id) }
            }
            div.detail-item {
                span.detail-label { "Original" }
                code { (svc.original_name) }
            }
        }

        section.content-section {
            div.section-heading {
                h2 { "Tool List" }
                span.section-count { (svc.tools.len()) " items" }
            }
            @if svc.tools.is_empty() {
                div.empty-state.compact {
                    h2 { "No tools found" }
                    p { "Tool definitions will appear here after connecting." }
                }
            } @else {
                div.tool-grid {
                    @for tool in &svc.tools {
                        (render_tool_card(&svc.name, tool))
                    }
                }
            }
        }

        section.content-section {
            div.section-heading {
                h2 { "Config" }
            }
            (config_block(&svc.config))
        }
    };

    Html(layout(&format!("mcpstore - {}", svc.name), content).into_string()).into_response()
}

pub(super) async fn page_add() -> impl IntoResponse {
    let content = html! {
        section.page-heading {
            div {
                p.eyebrow { "Add Service" }
                h1 { "New MCP Service" }
            }
            div.heading-actions {
                a.button.button-ghost href="/" { "Back" }
            }
        }

        form.add-form method="get" action="/add/exec" {
            div.form-grid {
                div.field {
                    label for="field-name" { "Name" }
                    input id="field-name" name="name" type="text" placeholder="github" required;
                }
                div.field {
                    label for="field-scope" { "Scope" }
                    select id="field-scope" name="scope" {
                        option value="store" { "Store" }
                        option value="agent" { "Agent" }
                    }
                }
                div.field {
                    label for="field-agent" { "Agent ID" }
                    input id="field-agent" name="agent" type="text" placeholder="agent-a";
                }
                div.field {
                    label for="field-transport" { "Transport" }
                    select id="field-transport" name="transport" {
                        option value="stdio" { "stdio" }
                        option value="streamable-http" { "streamable-http" }
                        option value="sse" { "sse" }
                    }
                }
                div.field.field-wide {
                    label for="field-command" { "Command or URL" }
                    input id="field-command" name="command_or_url" type="text" placeholder="npx -y @modelcontextprotocol/server-filesystem ." required;
                }
                div.field.field-wide {
                    label for="field-desc" { "Description" }
                    input id="field-desc" name="description" type="text" placeholder="Filesystem MCP service";
                }
                div.field.field-wide {
                    label for="field-working-dir" { "Working directory" }
                    input id="field-working-dir" name="working_dir" type="text" placeholder="/path/to/project";
                }
                div.field {
                    label for="field-env" { "Env vars" }
                    textarea id="field-env" name="env" placeholder="TOKEN=abc" {}
                }
                div.field {
                    label for="field-headers" { "Headers" }
                    textarea id="field-headers" name="headers" placeholder="Authorization=Bearer token" {}
                }
            }
            footer.form-footer {
                button.button.button-primary type="submit" { "Add" }
                a.button.button-ghost href="/" { "Cancel" }
            }
        }
    };
    Html(layout("mcpstore - Add Service", content).into_string())
}
