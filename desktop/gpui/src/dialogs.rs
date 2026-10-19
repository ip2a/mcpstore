//! Dialog entities: tool runner (schema-driven args form) and add-service form.

use crate::api::{Api, ServiceAddress, ToolInfo};
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
    default: Option<String>,
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
                default,
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

pub struct AddServiceDialog {
    api: Arc<Api>,
    name: Entity<InputState>,
    transport: Entity<InputState>,
    target: Entity<InputState>,
    description: Entity<InputState>,
    error: Option<SharedString>,
    done: bool,
    running: bool,
}

impl AddServiceDialog {
    pub fn open(api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        let entity = cx.new(|cx| Self {
            api,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("service name")),
            transport: cx.new(|cx| {
                InputState::new(window, cx).placeholder("stdio | streamable-http")
            }),
            target: cx.new(|cx| {
                InputState::new(window, cx).placeholder("command or url, e.g. uvx mcp-server-fs")
            }),
            description: cx.new(|cx| InputState::new(window, cx).placeholder("optional")),
            error: None,
            done: false,
            running: false,
        });
        let entity2 = entity.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Add service").child(entity2.clone())
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        let transport = self.transport.read(cx).value().trim().to_string();
        let target = self.target.read(cx).value().trim().to_string();
        let description = self.description.read(cx).value().trim().to_string();
        if name.is_empty() || target.is_empty() || transport.is_empty() {
            self.error = Some("name, transport and target are required".into());
            cx.notify();
            return;
        }
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    api.add_service(
                        &name,
                        &transport,
                        &target,
                        if description.is_empty() {
                            None
                        } else {
                            Some(description.as_str())
                        },
                    )
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

impl Render for AddServiceDialog {
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
                        .child("Service added. The list refreshes within 5 seconds."),
                )
            })
            .child(
                h_flex().gap_2().justify_end().child(
                    Button::new("add-service-submit")
                        .primary()
                        .label(if self.running { "Adding…" } else { "Add" })
                        .disabled(self.running || self.done)
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                ),
            )
    }
}
