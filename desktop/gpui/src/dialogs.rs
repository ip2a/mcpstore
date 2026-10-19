//! Dialog entities: tool runner (schema-driven args form) and add-service form.

use crate::api::{Api, ScopeRef, ServiceAddress, ServiceInstance, ToolInfo};
use gpui_kit::component::{
    WindowExt as _,
    button::Button,
    input::{Input, InputState},
    spinner::Spinner,
    ActiveTheme,
};
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use serde_json::Value;
use std::sync::Arc;

/// One field derived from a JSON-schema property.
struct SchemaField {
    name: String,
    required: bool,
    kind: String,
    description: Option<String>,
    input: Entity<InputState>,
}

fn extract_fields(
    schema: &Value,
    window: &mut Window,
    cx: &mut Context<ToolRunDialog>,
) -> Vec<SchemaField> {
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    props
        .into_iter()
        .map(|(name, spec)| {
            let kind = spec
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string")
                .to_string();
            let description = spec
                .get("description")
                .and_then(|d| d.as_str())
                .map(str::to_string);
            let default = spec
                .get("default")
                .map(|d| match d {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(match kind.as_str() {
                    "number" | "integer" => "number",
                    "boolean" => "true | false",
                    "object" | "array" => "{\"json\": …}",
                    _ => "text",
                });
                if let Some(default) = &default {
                    state = state.default_value(default.clone());
                }
                state
            });
            SchemaField {
                required: required.contains(&name),
                name,
                kind,
                description,
                input,
            }
        })
        .collect()
}

impl SchemaField {
    /// Parse the text input into a JSON value per the declared schema type.
    fn parse(&self, cx: &gpui_kit::App) -> anyhow::Result<(String, Value)> {
        let text = self.input.read(cx).value().trim().to_string();
        let value = match self.kind.as_str() {
            "number" => {
                let n: f64 = text
                    .parse()
                    .map_err(|_| anyhow::anyhow!("{}: {} is not a number", self.name, text))?;
                Value::Number(
                    serde_json::Number::from_f64(n)
                        .ok_or_else(|| anyhow::anyhow!("{}: {} out of range", self.name, text))?,
                )
            }
            "integer" => Value::Number(
                text.parse::<i64>()
                    .map_err(|_| anyhow::anyhow!("{}: {} is not an integer", self.name, text))?
                    .into(),
            ),
            "boolean" => match text.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => anyhow::bail!("{}: expected true or false", self.name),
            },
            "object" | "array" => {
                if text.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_str(&text)
                        .map_err(|e| anyhow::anyhow!("{}: invalid JSON ({e})", self.name))?
                }
            }
            _ => Value::String(text),
        };
        Ok((self.name.clone(), value))
    }
}

pub struct ToolRunDialog {
    addr: ServiceAddress,
    api: Arc<Api>,
    tool: ToolInfo,
    fields: Vec<SchemaField>,
    result: Option<SharedString>,
    error: Option<SharedString>,
    running: bool,
}

impl ToolRunDialog {
    pub fn open(
        addr: ServiceAddress,
        tool: ToolInfo,
        api: Arc<Api>,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let entity = cx.new(|cx| {
            let fields = extract_fields(&tool.input_schema, window, cx);
            Self {
                addr,
                api,
                tool,
                fields,
                result: None,
                error: None,
                running: false,
            }
        });
        let title: SharedString = format!("Run · {}", entity.read(cx).tool.name).into();
        let entity2 = entity.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title.clone())
                .child(entity2.clone())
                .footer(
                    h_flex().justify_end().child(
                        Button::new("tool-dialog-close")
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let mut args = serde_json::Map::new();
        for field in &self.fields {
            match field.parse(cx) {
                Ok((name, value)) => {
                    if !value.is_null() {
                        args.insert(name, value);
                    }
                }
                Err(e) => {
                    self.error = Some(format!("{e}").into());
                    cx.notify();
                    return;
                }
            }
        }
        self.running = true;
        self.error = None;
        self.result = None;
        cx.notify();
        let api = self.api.clone();
        let addr = self.addr.clone();
        let tool_name = self.tool.name.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.call_tool(&addr, &tool_name, args.into()) })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(value) => {
                        this.result = Some(
                            serde_json::to_string_pretty(&value)
                                .unwrap_or_else(|_| format!("{value}"))
                                .into(),
                        );
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ToolRunDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (muted, fg) = (cx.theme().muted_foreground, cx.theme().foreground);
        let mut form = v_flex().gap_3();
        for i in 0..self.fields.len() {
            let input = self.fields[i].input.clone();
            form = form.child(
                v_flex()
                    .gap_1()
                    .child(
                        div().text_sm().text_color(muted).child({
                            let mut label = self.fields[i].name.clone();
                            if self.fields[i].required {
                                label.push_str(" *");
                            }
                            h_flex()
                                .gap_2()
                                .child(label)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted.opacity(0.7))
                                        .child(format!("({})", self.fields[i].kind)),
                                )
                        }),
                    )
                    .child(Input::new(&input))
                    .when_some(
                        self.fields[i].description.clone(),
                        |el, desc| el.child(div().text_xs().text_color(muted).child(desc)),
                    ),
            );
        }
        if self.fields.is_empty() {
            form = form.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("This tool takes no arguments."),
            );
        }

        v_flex()
            .gap_3()
            .text_color(fg)
            .child(form)
            .child(
                h_flex().justify_end().child(
                    Button::new("tool-run")
                        .primary()
                        .label(if self.running { "Running…" } else { "Run" })
                        .disabled(self.running)
                        .on_click(cx.listener(|this, _, _, cx| this.run(cx))),
                ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when(self.running, |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Spinner::new())
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Running…"),
                        ),
                )
            })
            .when_some(self.result.clone(), |el, result| {
                el.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .font_semibold()
                                .text_sm()
                                .text_color(muted)
                                .child("Result"),
                        )
                        .child(div().text_xs().child(result)),
                )
            })
    }
}

/// Add / edit service form dialog. Edit mode prefills and PUTs the scope descriptor.
pub struct ServiceFormDialog {
    api: Arc<Api>,
    mode: FormMode,
    name: Entity<InputState>,
    transport: Entity<InputState>,
    target: Entity<InputState>,
    description: Entity<InputState>,
    error: Option<SharedString>,
    done: bool,
    running: bool,
}

#[derive(Clone)]
pub enum FormMode {
    Add,
    Edit {
        name: String,
        scope: ScopeRef,
        transport: String,
        target: String,
        description: String,
    },
}

impl ServiceFormDialog {
    pub fn open_add(api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        Self::open(api, FormMode::Add, window, cx);
    }

    pub fn open_edit(
        api: Arc<Api>,
        name: String,
        scope: ScopeRef,
        svc: &ServiceInstance,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let (transport, target) = match (&svc.transport, svc.command.as_deref()) {
            (transport, Some(command)) if transport == "stdio" => {
                let target = format!(
                    "{} {}",
                    command,
                    config_args(&svc.effective_config).join(" ")
                );
                ("stdio".to_string(), target)
            }
            (transport, _) if transport == "stdio" => ("stdio".to_string(), command_or_empty(svc)),
            (transport, _) => (transport.clone(), svc.url.clone().unwrap_or_default()),
        };
        let description = svc
            .effective_config
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        Self::open(
            api,
            FormMode::Edit {
                name,
                scope,
                transport,
                target,
                description,
            },
            window,
            cx,
        );
    }

    fn open(api: Arc<Api>, mode: FormMode, window: &mut Window, cx: &mut gpui_kit::App) {
        let (name_val, transport_val, target_val, desc_val) = match &mode {
            FormMode::Add => (String::new(), "stdio".to_string(), String::new(), String::new()),
            FormMode::Edit {
                name,
                transport,
                target,
                description,
                ..
            } => (name.clone(), transport.clone(), target.clone(), description.clone()),
        };
        let entity = cx.new(|cx| Self {
            api,
            mode,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("service name").default_value(name_val)),
            transport: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("stdio | streamable-http")
                    .default_value(transport_val)
            }),
            target: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("command or url, e.g. uvx mcp-server-fs")
                    .default_value(target_val)
            }),
            description: cx.new(|cx| InputState::new(window, cx).placeholder("optional").default_value(desc_val)),
            error: None,
            done: false,
            running: false,
        });
        let entity2 = entity.clone();
        let title = match &entity2.read(cx).mode {
            FormMode::Add => SharedString::from("Add service"),
            FormMode::Edit { name, .. } => format!("Edit · {name}").into(),
        };
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(title.clone()).child(entity2.clone())
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        let transport = self.transport.read(cx).value().trim().to_string();
        let target = self.target.read(cx).value().trim().to_string();
        let description = self.description.read(cx).value().trim().to_string();
        if name.is_empty() || target.is_empty() {
            self.error = Some("name and target are required".into());
            cx.notify();
            return;
        }
        if transport != "stdio" && transport != "streamable-http" {
            self.error = Some("transport must be stdio or streamable-http".into());
            cx.notify();
            return;
        }
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        let mode = self.mode.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    match mode {
                        FormMode::Add => {
                            api.add_service(&name, &transport, &target, Some(&description))
                        }
                        FormMode::Edit { name, scope, .. } => {
                            api.update_service_scope(&name, &scope, &transport, &target, Some(&description))
                        }
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(_) => this.done = true,
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn command_or_empty(svc: &ServiceInstance) -> String {
    svc.command.clone().unwrap_or_default()
}

fn config_args(config: &Value) -> Vec<String> {
    config
        .get("args")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Render for ServiceFormDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (muted, fg) = (cx.theme().muted_foreground, cx.theme().foreground);
        let field = |label: &'static str, input: &Entity<InputState>| {
            v_flex()
                .gap_1()
                .child(div().text_sm().text_color(muted).child(label))
                .child(Input::new(input))
        };
        v_flex()
            .gap_3()
            .text_color(fg)
            .child(field("Name", &self.name))
            .child(field("Transport", &self.transport))
            .child(field("Command or URL", &self.target))
            .child(field("Description", &self.description))
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when(self.done, |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child("Saved. The list refreshes within 5 seconds."),
                )
            })
            .child(
                h_flex().gap_2().justify_end().child(
                    Button::new("service-form-submit")
                        .primary()
                        .label(if self.running { "Saving…" } else { "Save" })
                        .disabled(self.running || self.done)
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                ),
            )
    }
}

/// Tool policy editor: available tool names, one per line.
pub struct ToolPolicyDialog {
    addr: ServiceAddress,
    api: Arc<Api>,
    tools: Entity<InputState>,
    error: Option<SharedString>,
    done: bool,
    running: bool,
}

impl ToolPolicyDialog {
    pub fn open(addr: ServiceAddress, api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        let handle = window.window_handle();
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            // Prefill with the current override from the backend.
            let current = cx
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.get_tool_policy(&addr).ok() }
                })
                .await;
            let _ = handle.update(cx, |_, window, cx| {
                let text = current
                    .and_then(|v| {
                        v.get("available_tools")
                            .or_else(|| v.get("tools"))
                            .and_then(|t| t.as_array())
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|i| i.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                    })
                    .unwrap_or_default();
                Self::open_inner(addr, api, text, window, cx);
            });
        })
        .detach();
    }

    fn open_inner(
        addr: ServiceAddress,
        api: Arc<Api>,
        current: String,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let text = current;
        let entity = cx.new(|cx| Self {
            addr,
            api,
            tools: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("allowed tool names, one per line (empty = allow all)")
                    .default_value(text)
            }),
            error: None,
            done: false,
            running: false,
        });
        let entity2 = entity.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Tool policy").child(entity2.clone())
        });
    }

    fn save(&mut self, cx: &mut Context<Self>, clear: bool) {
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        let addr = self.addr.clone();
        let text = self.tools.read(cx).value().to_string();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let tools: Vec<String> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            let result = cx
                .background_spawn(async move {
                    if clear {
                        api.clear_tool_policy(&addr)
                    } else {
                        api.set_tool_policy(&addr, &tools)
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(_) => this.done = true,
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ToolPolicyDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("One tool name per line. Clear to remove the override."),
            )
            .child(Input::new(&self.tools))
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when(self.done, |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child("Policy saved."),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("policy-clear")
                            .danger()
                            .label("Clear")
                            .disabled(self.running || self.done)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx, true))),
                    )
                    .child(
                        Button::new("policy-save")
                            .primary()
                            .label(if self.running { "Saving…" } else { "Save" })
                            .disabled(self.running || self.done)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx, false))),
                    ),
            )
    }
}

/// Read one resource and show its content.
pub struct ResourceReadDialog {
    content: Option<SharedString>,
    error: Option<SharedString>,
}

impl ResourceReadDialog {
    pub fn open(addr: ServiceAddress, api: Arc<Api>, uri: String, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity = cx.new(|_| Self {
            content: None,
            error: None,
        });
        let fetch = entity.downgrade();
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.read_resource(&addr, &uri) })
                .await;
            fetch
                .update(cx, |this, cx| {
                    match result {
                        Ok(value) => {
                            this.content = Some(
                                serde_json::to_string_pretty(&value)
                                    .unwrap_or_else(|_| format!("{value}"))
                                    .into(),
                            );
                        }
                        Err(e) => this.error = Some(format!("{e:#}").into()),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
        let entity2 = entity.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Resource").child(entity2.clone())
        });
    }
}

impl Render for ResourceReadDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .text_color(cx.theme().foreground)
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when_some(self.content.clone(), |el, content| {
                el.child(div().text_xs().child(content))
            })
            .when(self.content.is_none() && self.error.is_none(), |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Spinner::new())
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Reading…")),
                )
            })
    }
}

/// Declare a service into an agent scope (PUT /services/:name/scopes/agents/:id).
pub struct DeclareScopeDialog {
    api: Arc<Api>,
    agent_id: String,
    name: Entity<InputState>,
    transport: Entity<InputState>,
    target: Entity<InputState>,
    error: Option<SharedString>,
    done: bool,
    running: bool,
}

impl DeclareScopeDialog {
    pub fn open(agent_id: String, api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity = cx.new(|cx| Self {
            api,
            agent_id: agent_id.clone(),
            name: cx.new(|cx| InputState::new(window, cx).placeholder("service name")),
            transport: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("stdio | streamable-http")
                    .default_value("stdio")
            }),
            target: cx.new(|cx| {
                InputState::new(window, cx).placeholder("command or url, e.g. uvx mcp-server-fs")
            }),
            error: None,
            done: false,
            running: false,
        });
        let entity2 = entity.clone();
        let title: SharedString = format!("Declare service · agent {agent_id}").into();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(title.clone()).child(entity2.clone())
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        let transport = self.transport.read(cx).value().trim().to_string();
        let target = self.target.read(cx).value().trim().to_string();
        if name.is_empty() || target.is_empty() {
            self.error = Some("name and target are required".into());
            cx.notify();
            return;
        }
        if transport != "stdio" && transport != "streamable-http" {
            self.error = Some("transport must be stdio or streamable-http".into());
            cx.notify();
            return;
        }
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        let agent_id = self.agent_id.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    api.declare_agent_scope(&name, &agent_id, &transport, &target)
                })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(_) => this.done = true,
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for DeclareScopeDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let field = |label: &'static str, input: &Entity<InputState>| {
            v_flex()
                .gap_1()
                .child(div().text_sm().text_color(muted).child(label))
                .child(Input::new(input))
        };
        v_flex()
            .gap_3()
            .text_color(cx.theme().foreground)
            .child(field("Service name", &self.name))
            .child(field("Transport", &self.transport))
            .child(field("Command or URL", &self.target))
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when(self.done, |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child("Declared. Refresh the scope view to see it."),
                )
            })
            .child(
                h_flex().gap_2().justify_end().child(
                    Button::new("declare-submit")
                        .primary()
                        .label(if self.running { "Declaring…" } else { "Declare" })
                        .disabled(self.running || self.done)
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                ),
            )
    }
}

/// Show a fetched JSON document (config view).
pub struct JsonViewDialog {
    content: Option<SharedString>,
    title: SharedString,
    error: Option<SharedString>,
}

impl JsonViewDialog {
    pub fn open_fetch(title: impl Into<SharedString>, fetch: impl FnOnce() -> anyhow::Result<serde_json::Value> + Send + 'static, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity = cx.new(|_| Self {
            content: None,
            title: title.into(),
            error: None,
        });
        let fetch_entity = entity.downgrade();
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            let result = cx.background_spawn(async move { fetch() }).await;
            fetch_entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(value) => {
                            this.content = Some(
                                serde_json::to_string_pretty(&value).unwrap_or_default().into(),
                            );
                        }
                        Err(e) => this.error = Some(format!("{e:#}").into()),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
        Self::mount(entity, window, cx);
    }

    fn mount(entity: Entity<Self>, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity2 = entity.clone();
        let title = entity.read(cx).title.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(title.clone()).child(entity2.clone())
        });
    }
}

impl Render for JsonViewDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .text_color(cx.theme().foreground)
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when_some(self.content.clone(), |el, content| {
                el.child(div().text_xs().child(content))
            })
            .when(self.content.is_none() && self.error.is_none(), |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Spinner::new())
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Loading…")),
                )
            })
    }
}

/// Import services from a local MCP client config (app face /client-config/import).
pub struct ImportClientDialog {
    api: Arc<Api>,
    client: Entity<InputState>,
    path: Entity<InputState>,
    names: Entity<InputState>,
    error: Option<SharedString>,
    done: Option<SharedString>,
    running: bool,
}

impl ImportClientDialog {
    pub fn open(api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity = cx.new(|cx| Self {
            api,
            client: cx.new(|cx| InputState::new(window, cx).placeholder("client, e.g. claude")),
            path: cx.new(|cx| InputState::new(window, cx).placeholder("/path/to/config.json")),
            names: cx.new(|cx| {
                InputState::new(window, cx).placeholder("service names to import, one per line")
            }),
            error: None,
            done: None,
            running: false,
        });
        let entity2 = entity.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Import client services").child(entity2.clone())
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let client = self.client.read(cx).value().trim().to_string();
        let path = self.path.read(cx).value().trim().to_string();
        let names: Vec<String> = self
            .names
            .read(cx)
            .value()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if client.is_empty() || path.is_empty() || names.is_empty() {
            self.error = Some("client, path and at least one name are required".into());
            cx.notify();
            return;
        }
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.import_client_services(&client, &path, &names) })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(value) => {
                        let imported = value
                            .get("imported")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        this.done = Some(format!("Imported {imported} services.").into());
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ImportClientDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let field = |label: &'static str, input: &Entity<InputState>| {
            v_flex()
                .gap_1()
                .child(div().text_sm().text_color(muted).child(label))
                .child(Input::new(input))
        };
        v_flex()
            .gap_3()
            .text_color(cx.theme().foreground)
            .child(field("Client", &self.client))
            .child(field("Config path", &self.path))
            .child(field("Service names", &self.names))
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when_some(self.done.clone(), |el, done| {
                el.child(div().text_sm().text_color(cx.theme().success).child(done))
            })
            .child(
                h_flex().gap_2().justify_end().child(
                    Button::new("import-submit")
                        .primary()
                        .label(if self.running { "Importing…" } else { "Import" })
                        .disabled(self.running)
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                ),
            )
    }
}
