//! Services dashboard: aggregate list (scope=root) + search + lifecycle actions.

use crate::api::{Api, ScopeView, ServiceAddress, ServiceInstance};
use gpui_kit::assets::IconName;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    spinner::Spinner,
    ActiveTheme, Icon,
};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Task, Window, div,
};
use std::sync::Arc;
use std::time::Duration;
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

/// Sent to MainWindow: user asked to open a service's detail page.
#[derive(Debug, Clone)]
pub struct SelectService(pub ServiceAddress);

pub struct ServicesView {
    api: Arc<Api>,
    query: Entity<InputState>,
    search: String,
    services: Vec<ServiceInstance>,
    loading: bool,
    error: Option<SharedString>,
    _poll: Task<anyhow::Result<()>>,
    _search_sub: Subscription,
}

impl EventEmitter<SelectService> for ServicesView {}

impl ServicesView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search services…"));
        let search_sub = cx.subscribe_in(&query, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.search = state.read(cx).value().to_string();
                cx.notify();
            }
        });
        let poll = cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            loop {
                let api = this.read_with(cx, |view, _| view.api.clone())?;
                let result = cx
                    .background_spawn(async move { api.list_services(&ScopeView::Root).await })
                    .await;
                apply_services(&this, cx, result);
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
            #[allow(unreachable_code)]
            anyhow::Ok(())
        });
        Self {
            api,
            query,
            search: String::new(),
            services: Vec::new(),
            loading: true,
            error: None,
            _poll: poll,
            _search_sub: search_sub,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.list_services(&ScopeView::Root).await })
                .await;
            apply_services(&this, cx, result);
        })
        .detach();
    }

    fn lifecycle(&mut self, addr: ServiceAddress, action: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.lifecycle(&addr, action).await }
                })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("{action} failed: {e:#}").into());
                this.loading = true;
                cx.notify();
            })
            .ok();
            // Refetch so row state reflects the action.
            let result = cx
                .background_spawn(async move { api.list_services(&ScopeView::Root).await })
                .await;
            apply_services(&this, cx, result);
        })
        .detach();
    }

    fn render_row(&self, svc: &ServiceInstance, cx: &mut Context<Self>) -> impl IntoElement {
        let addr = ServiceAddress {
            service_name: svc.service_name.clone(),
            scope: svc.scope.clone(),
        };
        let open_addr = addr.clone();
        let connect_addr = addr.clone();
        let restart_addr = addr.clone();
        let disconnect_addr = addr.clone();
        let running = svc.state.phase == "running";
        let stopped = svc.state.phase == "stopped";
        let row_id = format!("svc-{}", svc.instance_id);

        h_flex()
            .id(format!("{row_id}-row"))
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .font_medium()
                                    .text_color(cx.theme().foreground)
                                    .truncate()
                                    .child(svc.service_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(svc.scope.label()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(transport_label(svc)),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .items_center()
                    .child(health_chip(&svc.state.health, cx))
                    .child(
                        div()
                            .w_16()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(svc.state.phase.clone()),
                    )
                    .child(
                        div()
                            .w_16()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} {}", svc.tools.len(), crate::dict::t(cx, crate::dict::Key::ToolsCount))),
                    )
                    .child(
                        Button::new(format!("{row_id}-connect"))
                            .ghost()
                            .icon(IconName::Play)
                            .tooltip("Connect")
                            .disabled(running)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(connect_addr.clone(), "connect", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-restart"))
                            .ghost()
                            .icon(IconName::RotateCw)
                            .tooltip("Restart")
                            .disabled(stopped)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(restart_addr.clone(), "restart", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-disconnect"))
                            .ghost()
                            .icon(IconName::Square)
                            .tooltip("Disconnect")
                            .disabled(stopped)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(disconnect_addr.clone(), "disconnect", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-edit"))
                            .ghost()
                            .icon(IconName::Pencil)
                            .tooltip("Edit")
                            .on_click({
                                let api = self.api.clone();
                                let svc = svc.clone();
                                move |_, window, cx| {
                                    crate::dialogs::ServiceFormDialog::open_edit(
                                        api.clone(),
                                        svc.service_name.clone(),
                                        svc.scope.clone(),
                                        &svc,
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    )
                    .child({
                        let api = self.api.clone();
                        let name = svc.service_name.clone();
                        let scope = svc.scope.clone();
                        Button::new(format!("{row_id}-remove"))
                            .ghost()
                            .danger()
                            .icon(IconName::Trash)
                            .tooltip(crate::dict::t(cx, crate::dict::Key::Remove))
                            .on_click({
                                let title = crate::dict::t(cx, crate::dict::Key::RemoveScopeTitle);
                                let body = crate::dict::t(cx, crate::dict::Key::RemoveScopeBody);
                                move |_, window, cx| {
                                    let api = api.clone();
                                    let name = name.clone();
                                    let scope = scope.clone();
                                    let title = title;
                                    let body = body.to_string();
                                    window.open_alert_dialog(cx, move |alert, _, _| {
                                        let api = api.clone();
                                        let name = name.clone();
                                        let scope = scope.clone();
                                        let title = title;
                                        let body = body.clone();
                                        alert
                                            .title(format!("{title} ({name})"))
                                            .description(body)
                                            .on_ok(move |_, _, cx| {
                                                let api = api.clone();
                                                let name = name.clone();
                                                let scope = scope.clone();
                                                cx.spawn(
                                                    async move |cx: &mut gpui_kit::AsyncApp| {
                                                        let _ = cx.background_spawn(async move {
                                                            api.remove_service_scope(&name, &scope).await
                                                        }).await;
                                                    },
                                                )
                                                .detach();
                                                true
                                            })
                                    });
                                }
                            })
                    })
                    .child(
                        Button::new(format!("{row_id}-open"))
                            .ghost()
                            .icon(IconName::ChevronLeft)
                            .tooltip("Details")
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                cx.emit(SelectService(open_addr.clone()));
                            })),
                    ),
            )
    }
}

impl Render for ServicesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.query.clone();
        let rows: Vec<ServiceInstance> = self
            .services
            .iter()
            .filter(|s| {
                self.search.is_empty()
                    || s.service_name
                        .to_lowercase()
                        .contains(&self.search.to_lowercase())
            })
            .cloned()
            .collect();

        let mut list = v_flex().size_full();
        for svc in &rows {
            list = list.child(self.render_row(svc, cx));
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
                    .child(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .child(div().flex_1().min_w_0().child(Input::new(&query)))
                    .child(
                        Button::new("services-refresh")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .label(crate::dict::t(cx, crate::dict::Key::Refresh))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("services-add")
                            .primary()
                            .icon(IconName::Plus)
                            .label(crate::dict::t(cx, crate::dict::Key::Add))
                            .on_click(cx.listener(|this, _, window, cx| {
                                crate::dialogs::ServiceFormDialog::open_add(this.api.clone(), window, cx);
                            })),
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
            .when(self.loading && rows.is_empty(), |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .justify_center()
                        .py_8()
                        .child(Spinner::new())
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::dict::t(cx, crate::dict::Key::Loading)),
                        ),
                )
            })
            .when(!self.loading && rows.is_empty(), |el| {
                el.child(
                    h_flex().justify_center().py_8().child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::dict::t(cx, crate::dict::Key::NoServices)),
                    ),
                )
            })
            .child(
                div()
                    .id("services-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(list),
            )
    }
}

fn apply_services(
    this: &gpui_kit::WeakEntity<ServicesView>,
    cx: &mut gpui_kit::AsyncApp,
    result: anyhow::Result<Vec<ServiceInstance>>,
) {
    this.update(cx, |this, cx| {
        this.loading = false;
        match result {
            Ok(services) => {
                this.services = services;
                this.error = None;
            }
            Err(e) => this.error = Some(format!("{e:#}").into()),
        }
        cx.notify();
    })
    .ok();
}

fn transport_label(svc: &ServiceInstance) -> String {
    match (svc.url.as_deref(), svc.command.as_deref()) {
        (_, Some(command)) if svc.transport == "stdio" => format!("stdio: {command}"),
        (Some(url), _) => format!("{}: {url}", svc.transport),
        _ => svc.transport.clone(),
    }
}

fn health_chip(health: &str, cx: &Context<'_, ServicesView>) -> impl IntoElement {
    let (color, label) = match health {
        "healthy" => (cx.theme().success, "healthy"),
        "degraded" => (cx.theme().warning, "degraded"),
        "unhealthy" => (cx.theme().danger, "unhealthy"),
        _ => (cx.theme().muted_foreground, "unknown"),
    };
    div()
        .rounded(cx.theme().radius)
        .px_2()
        .py_0p5()
        .text_xs()
        .bg(color.opacity(0.15))
        .text_color(color)
        .child(label)
}
