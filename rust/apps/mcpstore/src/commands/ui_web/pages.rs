use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use maud::html;
use mcpstore::state::{DesiredState, ReadinessStatus};
use mcpstore::{CacheStorage, InstanceId, MCPStore, ScopeRef};
use std::{collections::BTreeSet, collections::HashMap, sync::Arc};

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

    let all_instances = store.list_instances().await;
    let agents = all_instances
        .iter()
        .filter_map(|instance| instance.scope.agent_id().map(str::to_string))
        .collect::<BTreeSet<_>>();
    let selected_scope = if agent_filter.is_empty() || agent_filter == "store" {
        ScopeRef::Store
    } else {
        ScopeRef::Agent {
            agent_id: agent_filter.clone(),
        }
    };
    let mut services = all_instances
        .into_iter()
        .filter(|instance| instance.scope == selected_scope)
        .collect::<Vec<_>>();
    services.sort_by(|a, b| {
        a.service_name
            .cmp(&b.service_name)
            .then_with(|| a.instance_id.cmp(&b.instance_id))
    });
    let mut service_states = Vec::with_capacity(services.len());
    for service in &services {
        match store.service_state_entry(service.instance_id).await {
            Ok(state) => service_states.push(state),
            Err(error) => {
                return Html(
                    layout("mcpstore - Error", error_markup(&error.to_string())).into_string(),
                )
                .into_response();
            }
        }
    }

    let total = services.len();
    let ready = service_states
        .iter()
        .filter(|state| state.readiness.status == ReadinessStatus::Ready)
        .count();
    let not_ready = service_states
        .iter()
        .filter(|state| state.readiness.status == ReadinessStatus::NotReady)
        .count();
    let unknown = service_states
        .iter()
        .filter(|state| state.readiness.status == ReadinessStatus::Unknown)
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
                span.metric-value { (ready) }
                span.metric-label { "Ready" }
            }
            div.metric-card {
                span.metric-value { (not_ready) }
                span.metric-label { "Not ready" }
            }
            @if unknown > 0 {
                div.metric-card.metric-warning {
                    span.metric-value { (unknown) }
                    span.metric-label { "Unknown" }
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
                        @for agent_id in &agents {
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
                    @for (svc, state) in services.iter().zip(&service_states) {
                        (render_service_row(svc, state))
                    }
                }
            }
        }
    };

    Html(layout("mcpstore", content).into_string()).into_response()
}

pub(super) async fn page_service(
    State(store): State<Arc<MCPStore>>,
    Path(instance_id): Path<InstanceId>,
) -> impl IntoResponse {
    let svc = match store.find_instance(instance_id).await {
        Some(s) => s,
        None => {
            return Html(
                layout("mcpstore - Error", error_markup("Service not found")).into_string(),
            )
            .into_response();
        }
    };
    let state = match store.service_state_entry(instance_id).await {
        Ok(state) => state,
        Err(error) => {
            return Html(layout("mcpstore - Error", error_markup(&error.to_string())).into_string())
                .into_response()
        }
    };
    let instance_segment = svc.instance_id.to_string();
    let added = format_added_time(svc.added_time);
    let endpoint = svc.url.as_deref().or(svc.command.as_deref()).unwrap_or("-");

    let content = html! {
        section.page-heading {
            div {
                p.eyebrow { (svc.transport) }
                h1 { (svc.service_name) }
                div.service-meta {
                    (status_badge(state.readiness.status))
                    span.meta-pill { "tools · " (svc.tools.len()) }
                    span.meta-pill { "added · " (added) }
                }
            }
            div.heading-actions {
                a.button.button-ghost href="/" { "Back" }
                @if state.desired == DesiredState::Running {
                    a.button href=(format!("/action/disconnect/{instance_segment}")) { "Disconnect" }
                } @else {
                    a.button.button-primary href=(format!("/action/connect/{instance_segment}")) { "Connect" }
                }
                a.button href=(format!("/action/restart/{instance_segment}")) { "Restart" }
                a.button.button-danger href=(format!("/action/remove/{instance_segment}")) onclick="return confirm('Confirm delete service scope?')" { "Delete" }
            }
        }

        section.detail-grid {
            div.detail-item {
                span.detail-label { "Name" }
                code { (svc.service_name) }
            }
            div.detail-item {
                span.detail-label { "Endpoint" }
                code { (endpoint) }
            }
            div.detail-item {
                span.detail-label { "Scope" }
                code { (format!("{:?}", svc.scope)) }
            }
            div.detail-item {
                span.detail-label { "Instance ID" }
                code { (svc.instance_id) }
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
                        (render_tool_card(&instance_segment, tool))
                    }
                }
            }
        }

        section.content-section {
            div.section-heading {
                h2 { "Config" }
            }
            (config_block(&serde_json::Value::Object(svc.effective_config.clone())))
        }
    };

    Html(layout(&format!("mcpstore - {}", svc.service_name), content).into_string()).into_response()
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
