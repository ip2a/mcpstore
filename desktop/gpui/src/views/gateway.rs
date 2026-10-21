//! Gateway page: connection card, expose configuration, live routed-call log.

use crate::api::Api;
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{spinner::Spinner, ActiveTheme};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use serde_json::Value;
use std::sync::Arc;
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;

const EXPOSE_FAMILIES: [(&str, usize); 8] = [
    ("session_state", 0),
    ("tool", 1),
    ("prompt", 2),
    ("resource", 3),
    ("openapi", 4),
    ("service", 5),
    ("cache", 6),
    ("search", 7),
];

pub struct GatewayView {
    api: Arc<Api>,
    status: Option<Value>,
    requests: Vec<Value>,
    latest_seq: u64,
    wired: Vec<Value>,
    wire_client: String,
    wire_path: Entity<InputState>,
    scope: String,
    agent_id: Entity<InputState>,
    expose: [bool; 8],
    loading: bool,
    polling: bool,
    error: Option<SharedString>,
}

const WIRE_CLIENTS: [&str; 4] = ["claude-code", "cursor", "opencode", "codex"];

impl GatewayView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            api,
            status: None,
            requests: Vec::new(),
            latest_seq: 0,
            wired: Vec::new(),
            wire_client: "claude-code".into(),
            wire_path: cx.new(|cx| {
                InputState::new(window, cx).placeholder("config file path, e.g. ~/.claude.json")
            }),
            scope: "store".into(),
            agent_id: cx.new(|cx| InputState::new(window, cx).placeholder("agent_id")),
            expose: [false; 8],
            loading: true,
            polling: false,
            error: None,
        };
        view.refresh(cx);
        view
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let status = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_status("?scope=store").await }
                })
                .await;
            let requests = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_requests(0, 0).await }
                })
                .await;
            let wired = cx
                .background_spawn(async move { api.gateway_wired().await })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match status {
                    Ok(value) => {
                        this.error = None;
                        this.status = Some(value);
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                if let Ok(value) = requests {
                    this.requests = value
                        .get("requests")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    this.latest_seq = value
                        .get("latest_seq")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(this.latest_seq);
                }
                if let Ok(value) = wired {
                    this.wired = value.as_array().cloned().unwrap_or_default();
                }
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.maybe_poll_requests(cx)).ok();
        })
        .detach();
    }

    /// Long-poll the routed-call log (`after=<seq>&wait_ms=7000`) while the gateway runs;
    /// records arrive incrementally and a quiet gateway costs one blocked request per cycle.
    fn maybe_poll_requests(&mut self, cx: &mut Context<Self>) {
        if self.polling || !self.running() {
            return;
        }
        self.polling = true;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let mut after: u64 = 0;
            for _ in 0..1000 {
                let requests = cx
                    .background_spawn({
                        let api = api.clone();
                        async move { api.gateway_requests(after, 7_000).await }
                    })
                    .await;
                let mut still_running = false;
                let mut fresh = false;
                this.update(cx, |this, cx| {
                    if let Ok(value) = requests {
                        still_running = value
                            .get("running")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let new_records = value
                            .get("requests")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        after = value
                            .get("latest_seq")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(after);
                        fresh = !new_records.is_empty();
                        if fresh {
                            this.requests.extend(new_records);
                            let len = this.requests.len();
                            if len > 512 {
                                let drop = len - 512;
                                this.requests.drain(0..drop);
                            }
                        }
                    }
                    if fresh {
                        cx.notify();
                    }
                })
                .ok();
                if !still_running {
                    break;
                }
            }
            this.update(cx, |this, _| this.polling = false).ok();
        })
        .detach();
    }

    fn running(&self) -> bool {
        self.status
            .as_ref()
            .and_then(|s| s.get("running"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn url(&self) -> String {
        self.status
            .as_ref()
            .and_then(|s| s.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Query string for start/descriptor from the current form state.
    fn launch_query(&self, cx: &Context<Self>) -> String {
        let mut q = if self.scope == "agent" {
            let agent = self.agent_id.read(cx).value().trim().to_string();
            if agent.is_empty() {
                "?scope=agent".to_string()
            } else {
                format!("?scope=agent&agent_id={}", urlencoding::encode(&agent))
            }
        } else {
            "?scope=store".to_string()
        };
        for (name, index) in EXPOSE_FAMILIES {
            if self.expose[index] {
                q.push_str(&format!("&{name}=true"));
            }
        }
        q
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        let query = self.launch_query(cx);
        let agent = self.agent_id.read(cx).value().trim().to_string();
        let scope_ok = self.scope != "agent" || !agent.is_empty();
        if !scope_ok {
            self.error = Some("agent scope needs an agent_id".into());
            self.loading = false;
            cx.notify();
            return;
        }
        let was_running = self.running();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            if was_running {
                let _ = cx
                    .background_spawn({
                        let api = api.clone();
                        async move { api.gateway_action("stop", "").await }
                    })
                    .await;
            }            let result = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_action("start", &query).await }
                })
                .await;
            let status = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_status("?scope=store").await }
                })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                if let Err(e) = result {
                    this.error = Some(format!("start failed: {e:#}").into());
                }
                if let Ok(value) = status {
                    this.status = Some(value);
                }
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.maybe_poll_requests(cx)).ok();
        })
        .detach();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let _ = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_action("stop", "").await }
                })
                .await;
            let status = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_status("?scope=store").await }
                })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                if let Ok(value) = status {
                    this.status = Some(value);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cycle_scope(&mut self, cx: &mut Context<Self>) {
        self.scope = if self.scope == "store" { "agent" } else { "store" }.into();
        cx.notify();
    }

    fn cycle_wire_client(&mut self, cx: &mut Context<Self>) {
        let index = WIRE_CLIENTS
            .iter()
            .position(|c| *c == self.wire_client)
            .map(|i| (i + 1) % WIRE_CLIENTS.len())
            .unwrap_or(0);
        self.wire_client = WIRE_CLIENTS[index].into();
        cx.notify();
    }

    fn wire(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let client = self.wire_client.clone();
        let path = self.wire_path.read(cx).value().trim().to_string();
        if path.is_empty() {
            self.error = Some("wire needs a config file path".into());
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn({
                    let api = api.clone();
                    let client = client.clone();
                    async move { api.gateway_wire(&client, &path).await }
                })
                .await;
            let wired = cx
                .background_spawn(async move { api.gateway_wired().await })
                .await;
            this.update(cx, |this, cx| {
                if let Err(e) = result {
                    this.error = Some(format!("wire failed: {e:#}").into());
                }
                if let Ok(value) = wired {
                    this.wired = value.as_array().cloned().unwrap_or_default();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn unwire(&mut self, client: String, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn({
                    let api = api.clone();
                    let client = client.clone();
                    async move { api.gateway_unwire(&client).await }
                })
                .await;
            let wired = cx
                .background_spawn(async move { api.gateway_wired().await })
                .await;
            this.update(cx, |this, cx| {
                if let Err(e) = result {
                    this.error = Some(format!("unwire failed: {e:#}").into());
                }
                if let Ok(value) = wired {
                    this.wired = value.as_array().cloned().unwrap_or_default();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_expose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.expose[index] = !self.expose[index];
        cx.notify();
    }

    fn copy_url(&mut self, cx: &mut Context<Self>) {
        let url = self.url();
        if !url.is_empty() {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(url));
        }
    }
}

fn log_row(entry: &Value, cx: &Context<GatewayView>) -> gpui_kit::AnyElement {
    let theme = cx.theme();
    let tool = entry.get("tool").and_then(|v| v.as_str()).unwrap_or("?");
    let instance = entry
        .get("instance_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let duration = entry
        .get("duration_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or_default();
    let success = entry
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or_default();
    let at_unix_ms = entry.get("at_unix_ms").and_then(|v| v.as_u64()).unwrap_or_default();
    let secs = at_unix_ms / 1000;
    // UTC wall clock, no chrono dep; close enough for a log column.
    let time = format!("{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    h_flex()
        .gap_2()
        .px_3()
        .py_1()
        .border_b_1()
        .border_color(theme.border)
        .child(div().w_16().text_xs().font_family("Monaco").text_color(theme.muted_foreground).child(time))
        .child(div().w_6().text_xs().child(if success { "✓" } else { "✗" }).text_color(if success { theme.success } else { theme.danger }))
        .child(div().flex_1().min_w_0().truncate().text_sm().child(tool.to_string()))
        .child(div().w_40().truncate().text_xs().font_family("Monaco").text_color(theme.muted_foreground).child(instance.to_string()))
        .child(div().w_16().text_xs().font_family("Monaco").text_color(theme.muted_foreground).child(format!("{duration}ms")))
        .into_any_element()
}

impl Render for GatewayView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let running = self.running();
        let url = self.url();
        let status_chip = if self.loading {
            "…"
        } else if running {
            "running"
        } else {
            "stopped"
        };

        let mut expose_buttons = h_flex().gap_2().flex_wrap();
        for (name, index) in EXPOSE_FAMILIES {
            expose_buttons = expose_buttons.child(
                Button::new(format!("gw-expose-{name}"))
                    .outline()
                    .label(name)
                    .when(self.expose[index], |b| b.primary())
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_expose(index, cx))),
            );
        }

        let mut wired_rows = v_flex().gap_0p5();
        for entry in &self.wired {
            let client = entry.get("client").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let url = entry.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            wired_rows = wired_rows.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_1()
                    .child(div().w_24().text_sm().child(client.clone()))
                    .child(div().w_48().truncate().text_xs().font_family("Monaco").text_color(muted).child(url))
                    .child(div().flex_1().min_w_0().truncate().text_xs().font_family("Monaco").text_color(muted).child(path))
                    .child(
                        Button::new(format!("gw-unwire-{client}"))
                            .ghost()
                            .danger()
                            .label("Unwire")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.unwire(client.clone(), cx)
                            })),
                    ),
            );
        }

        let mut log_rows = v_flex();
        for entry in self.requests.iter().rev() {
            log_rows = log_rows.child(log_row(entry, cx));
        }

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().font_semibold().child("Gateway"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if running {
                                cx.theme().success
                            } else {
                                muted
                            })
                            .child(status_chip),
                    )
                    .when(!url.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .font_family("Monaco")
                                .text_color(muted)
                                .truncate()
                                .child(url.clone()),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("gw-copy")
                            .ghost()
                            .icon(IconName::Copy)
                            .label("Copy URL")
                            .disabled(url.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.copy_url(cx))),
                    )
                    .child(
                        Button::new("gw-refresh")
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(
                div()
                    .id("gateway-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_3()
                            // Connection
                            .child(
                                v_flex()
                                    .gap_2()
                                    .p_3()
                                    .border_1()
                                    .rounded_md()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child("Connection — one HTTP MCP endpoint for every client"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("gw-start")
                                                    .primary()
                                                    .label("Start")
                                                    .disabled(self.loading)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.start(cx)
                                                    })),
                                            )
                                            .child(
                                                Button::new("gw-stop")
                                                    .outline()
                                                    .danger()
                                                    .label("Stop")
                                                    .disabled(!running || self.loading)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.stop(cx)
                                                    })),
                                            ),
                                    ),
                            )
                            // Expose configuration
                            .child(
                                v_flex()
                                    .gap_2()
                                    .p_3()
                                    .border_1()
                                    .rounded_md()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child(
                                                "Exposure — scope + management tool families (applies on Start)",
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                Button::new("gw-scope")
                                                    .outline()
                                                    .label(format!("scope: {}", self.scope))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.cycle_scope(cx)
                                                    })),
                                            )
                                            .when(self.scope == "agent", |el| {
                                                el.child(
                                                    div().w_56().child(Input::new(&self.agent_id)),
                                                )
                                            }),
                                    )
                                    .child(expose_buttons),
                            )
                            // Connect clients (write-back)
                            .child(
                                v_flex()
                                    .gap_2()
                                    .p_3()
                                    .border_1()
                                    .rounded_md()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child(
                                                "Connect clients — writes the gateway URL into the client's MCP config",
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("gw-wire-client")
                                                    .outline()
                                                    .label(self.wire_client.clone())
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.cycle_wire_client(cx)
                                                    })),
                                            )
                                            .child(
                                                div().flex_1().child(Input::new(&self.wire_path)),
                                            )
                                            .child(
                                                Button::new("gw-wire")
                                                    .primary()
                                                    .label("Wire")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.wire(cx)
                                                    })),
                                            ),
                                    )
                                    .when(!self.wired.is_empty(), |el| el.child(wired_rows)),
                            )
                            // Routed-call log
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .font_semibold()
                                                    .text_sm()
                                                    .text_color(muted)
                                                    .child("Requests"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(format!("{} routed calls", self.requests.len())),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .border_1()
                                            .rounded_md()
                                            .border_color(cx.theme().border)
                                            .overflow_hidden()
                                            .child(if self.requests.is_empty() {
                                                h_flex()
                                                    .gap_2()
                                                    .px_3()
                                                    .py_2()
                                                    .items_center()
                                                    .when(self.loading, |el| {
                                                        el.child(Spinner::new())
                                                    })
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .child(if running {
                                                                "no routed calls yet"
                                                            } else {
                                                                "gateway not running"
                                                            }),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                log_rows.into_any_element()
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}
