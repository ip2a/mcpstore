//! One service's detail page: state, tools, lifecycle actions.

use crate::api::{Api, ServiceAddress, ServiceInstance, ToolInfo};
use gpui_kit::component::{
    button::Button,
    spinner::Spinner,
    ActiveTheme, IconName,
};
use gpui_kit::{
    AppContext as _, Context, EventEmitter, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div,
};
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

/// Sent to MainWindow: go back to the services list.
#[derive(Debug)]
pub struct BackToServices;

pub struct InstanceView {
    addr: ServiceAddress,
    api: Arc<Api>,
    instance: Option<ServiceInstance>,
    tools: Vec<ToolInfo>,
    loading: bool,
    error: Option<SharedString>,
}

impl EventEmitter<BackToServices> for InstanceView {}

impl InstanceView {
    pub fn new(addr: ServiceAddress, api: Arc<Api>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            addr,
            api,
            instance: None,
            tools: Vec::new(),
            loading: true,
            error: None,
        };
        view.refresh(cx);
        view
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        let addr = self.addr.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let instance = cx
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.get_service(&addr) }
                })
                .await;
            let tools = cx
                .background_spawn(async move { api.list_tools(&addr) })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match instance {
                    Ok(instance) => {
                        this.instance = Some(instance);
                        this.error = None;
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                if let Ok(tools) = tools {
                    this.tools = tools;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn lifecycle(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let addr = self.addr.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.lifecycle(&addr, action) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("{action} failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn render_field(
        label: &'static str,
        value: String,
        muted: gpui_kit::Hsla,
        fg: gpui_kit::Hsla,
        border: gpui_kit::Hsla,
    ) -> impl IntoElement {
        h_flex()
            .gap_3()
            .py_1()
            .border_b_1()
            .border_color(border)
            .child(div().w_40().flex_shrink_0().text_sm().text_color(muted).child(label))
            .child(div().min_w_0().text_sm().text_color(fg).truncate().child(value))
    }
}

impl Render for InstanceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.instance.as_ref().is_some_and(|s| s.state.phase == "running");
        let stopped = self
            .instance
            .as_ref()
            .is_some_and(|s| s.state.phase == "stopped");
        let name = self.addr.service_name.clone();
        let (muted, fg, border) = (
    cx.theme().muted_foreground,
    cx.theme().foreground,
    cx.theme().border,
);
let mut fields = v_flex();

        if let Some(svc) = &self.instance {
            fields = fields
                .child(Self::render_field("Instance ID", svc.instance_id.clone(), muted, fg, border))
                .child(Self::render_field("Scope", svc.scope.label(), muted, fg, border))
                .child(Self::render_field("Transport", svc.transport.clone(), muted, fg, border))
                .child(Self::render_field(
                    "Target",
                    svc.url
                        .clone()
                        .or_else(|| svc.command.clone())
                        .unwrap_or_else(|| "—".into()),
                    muted,
                    fg,
                    border,
                ))
                .child(Self::render_field("Desired", svc.state.desired.clone(), muted, fg, border))
                .child(Self::render_field("Phase", svc.state.phase.clone(), muted, fg, border))
                .child(Self::render_field("Health", svc.state.health.clone(), muted, fg, border))
                .child(Self::render_field(
                    "Readiness",
                    format!("{} ({})", svc.state.readiness.status, svc.state.readiness.reason),
                    muted,
                    fg,
                    border,
                ));
            if let Some(failure) = &svc.state.failure {
                fields = fields.child(
                    div()
                        .my_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().danger)
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(format!(
                            "Failure [{}] {}: {}",
                            failure.phase, failure.code, failure.message
                        )),
                );
            }
        }

        let mut tools = v_flex();
        for tool in &self.tools {
            tools = tools.child(
                v_flex()
                    .gap_0p5()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .font_medium()
                            .text_sm()
                            .child(tool.title.clone().unwrap_or_else(|| tool.name.clone())),
                    )
                    .when_some(
                        (!tool.description.is_empty()).then(|| tool.description.clone()),
                        |el, desc| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(desc),
                            )
                        },
                    ),
            );
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
                    .child(
                        Button::new("instance-back")
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .on_click(cx.listener(|_this, _, _, cx| {
                                cx.emit(BackToServices);
                            })),
                    )
                    .child(
                        div()
                            .font_semibold()
                            .child(format!("Service · {name}")),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("instance-connect")
                            .outline()
                            .icon(IconName::Play)
                            .label("Connect")
                            .disabled(running)
                            .on_click(cx.listener(|this, _, _, cx| this.lifecycle("connect", cx))),
                    )
                    .child(
                        Button::new("instance-restart")
                            .outline()
                            .icon(IconName::RotateCw)
                            .label("Restart")
                            .disabled(stopped)
                            .on_click(cx.listener(|this, _, _, cx| this.lifecycle("restart", cx))),
                    )
                    .child(
                        Button::new("instance-disconnect")
                            .outline()
                            .icon(IconName::Square)
                            .label("Disconnect")
                            .disabled(stopped)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.lifecycle("disconnect", cx)
                            })),
                    )
                    .child(
                        Button::new("instance-refresh")
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
                    .id("instance-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_2()
                            .when(self.loading && self.instance.is_none(), |el| {
                                el.child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Spinner::new())
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Loading…"),
                                        ),
                                )
                            })
                            .child(fields)
                            .child(
                                div()
                                    .mt_3()
                                    .font_semibold()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Tools ({})", self.tools.len())),
                            )
                            .child(tools),
                    ),
            )
    }
}
