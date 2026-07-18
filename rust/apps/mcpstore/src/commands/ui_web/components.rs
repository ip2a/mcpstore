use maud::{html, Markup};
use mcpstore::registry::{ServiceInstance, ToolInfo};
use mcpstore::state::{ReadinessStatus, ServiceState};
use mcpstore::ScopeRef;
use std::collections::BTreeSet;

use super::utils::{pretty_json, status_meta, truncate_chars, url_component};

struct ParamSummary {
    name: String,
    ty: String,
    desc: String,
    required: bool,
}

pub(super) fn status_badge(status: ReadinessStatus) -> Markup {
    let meta = status_meta(status);
    html! {
        span class=(format!("status-badge {}", meta.class_name)) { (meta.label) }
    }
}

pub(super) fn render_service_table_header() -> Markup {
    html! {
        div.service-table-head aria-hidden="true" {
            span { "Service" }
            span { "Scope" }
            span { "Transport" }
            span { "Status" }
            span { "Tools" }
            span { "Actions" }
        }
    }
}

pub(super) fn render_service_row(svc: &ServiceInstance, state: &ServiceState) -> Markup {
    let description = svc
        .effective_config
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let desc_short = if description.is_empty() {
        "No description".to_string()
    } else {
        truncate_chars(description, 72)
    };
    let instance_segment = svc.instance_id.to_string();
    let scope = match &svc.scope {
        ScopeRef::Store => "store",
        ScopeRef::Agent { agent_id } => agent_id,
    };

    html! {
        div.service-row {
            div.service-main {
                a.service-name href=(format!("/service/{instance_segment}")) { (svc.service_name) }
                span.service-desc { (desc_short) }
            }
            span.service-group { (scope) }
            span.transport-pill { (svc.transport) }
            (status_badge(state.readiness.status))
            span.service-tools { (svc.tools.len()) }
            div.row-actions {
                a.button.button-ghost href=(format!("/service/{instance_segment}")) { "View" }
                @if state.desired == mcpstore::state::DesiredState::Running {
                    a.button href=(format!("/action/disconnect/{instance_segment}")) { "Disconnect" }
                } @else {
                    a.button.button-primary href=(format!("/action/connect/{instance_segment}")) { "Connect" }
                }
                a.button href=(format!("/action/restart/{instance_segment}")) { "Restart" }
                a.button.button-danger href=(format!("/action/remove/{instance_segment}")) onclick="return confirm('Confirm delete service scope?')" { "Delete" }
            }
        }
    }
}

pub(super) fn render_tool_card(instance_id: &str, tool: &ToolInfo) -> Markup {
    let tool_segment = url_component(&tool.name);
    let tool_json = serde_json::to_string(tool).unwrap_or_else(|_| "{}".to_string());
    let summary = schema_summary(&tool.input_schema);
    let description = if tool.description.is_empty() {
        "No description".to_string()
    } else {
        truncate_chars(&tool.description, 180)
    };

    html! {
        article.tool-card {
            header.tool-card-header {
                div.tool-title {
                    span.tool-name { (tool.name) }
                    span.tool-meta { (summary.len()) " params" }
                }
                div.tool-card-actions {
                    a.button.button-primary href="#" data-modal=(format!("/modal/call-tool/{instance_id}/{tool_segment}")) { "Run" }
                    a.button.button-ghost href="#" data-modal=(format!("/modal/tool-detail/{instance_id}/{tool_segment}")) { "Details" }
                    button.button.button-ghost type="button" data-copy=(tool_json) { "Copy" }
                }
            }
            p.tool-description { (description) }
            @if summary.is_empty() {
                div.param-empty { "No params required" }
            } @else {
                div.param-list {
                    @for param in &summary {
                        div.param-item {
                            div.param-main {
                                code.param-name { (param.name) }
                                span.param-type { (param.ty) }
                                @if param.required {
                                    span.param-required { "Required" }
                                }
                            }
                            @if !param.desc.is_empty() {
                                span.param-desc { (truncate_chars(&param.desc, 96)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn error_markup(msg: &str) -> Markup {
    html! {
        div.empty-state {
            h2 { "Operation failed" }
            p { (msg) }
            a.button.button-primary href="/" { "Back to home" }
        }
    }
}

pub(super) fn modal_notice(title: &str, msg: &str) -> Markup {
    html! {
        dialog open {
            article {
                header.modal-header {
                    h3 { (title) }
                    button.button.button-ghost type="button" onclick="closeModal()" { "Close" }
                }
                p.modal-message { (msg) }
                footer {
                    button.button.button-primary type="button" onclick="closeModal()" { "Close" }
                }
            }
        }
    }
}

pub(super) fn modal_frame(
    title: &str,
    subtitle: Option<&str>,
    body: Markup,
    footer_content: Option<Markup>,
) -> Markup {
    html! {
        dialog open {
            article {
                header.modal-header {
                    div {
                        h3 { (title) }
                        @if let Some(subtitle) = subtitle {
                            p.hint { (subtitle) }
                        }
                    }
                    button.button.button-ghost type="button" onclick="closeModal()" { "Close" }
                }
                (body)
                @if let Some(footer_content) = footer_content {
                    footer { (footer_content) }
                }
            }
        }
    }
}

pub(super) fn config_block(value: &serde_json::Value) -> Markup {
    let config_json = pretty_json(value);
    html! {
        pre.code-block { code { (config_json) } }
    }
}

fn schema_summary(schema: &serde_json::Value) -> Vec<ParamSummary> {
    let required: BTreeSet<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (name, value) in props {
            let ty = value
                .get("type")
                .and_then(|t| t.as_str())
                .or_else(|| value.get("enum").map(|_| "enum"))
                .unwrap_or("any");
            let desc = value
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            out.push(ParamSummary {
                name: name.clone(),
                ty: ty.to_string(),
                desc,
                required: required.contains(name),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
