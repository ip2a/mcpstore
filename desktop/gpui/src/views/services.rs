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
use gpui_kit::StatefulInteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

/// Sent to MainWindow: user asked to open a service's detail page.
#[derive(Debug, Clone)]
pub struct SelectService(pub ServiceAddress);

pub struct ServicesView {
    api: Arc<Api>,
    query: Entity<InputState>,
    search: String,
    phase_filter: String,
    /// User order of `instance_id`. Refreshes never reshuffle this.
    order: Vec<String>,
    services: Vec<ServiceInstance>,
    loading: bool,
    error: Option<SharedString>,
    _poll: Task<anyhow::Result<()>>,
    _search_sub: Subscription,
}

impl EventEmitter<SelectService> for ServicesView {}

impl ServicesView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::dict::t(cx, crate::dict::TextKey::SearchServices))
        });
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
            phase_filter: String::new(),
            order: load_order(),
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

    fn check(&mut self, addr: ServiceAddress, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.check_service(&addr).await }
                })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("check failed: {e:#}").into());
                this.loading = true;
                cx.notify();
            })
            .ok();
            // Refetch so row state reflects the probe result.
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

    /// One-shot. Does not stay on, so later status changes do not re-sort.
    fn sort_by_name(&mut self, cx: &mut Context<Self>) {
        if self.services.is_empty() {
            return;
        }
        let mut ranked: Vec<(String, String)> = self
            .services
            .iter()
            .map(|svc| (svc.service_name.clone(), svc.instance_id.clone()))
            .collect();
        ranked.sort();
        self.order = ranked.into_iter().map(|(_, id)| id).collect();
        persist_order(&self.order);
        cx.notify();
    }

    fn move_row(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        move_id(&mut self.order, from, to);
        persist_order(&self.order);
        cx.notify();
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
        let check_addr = addr.clone();
        let running = svc.state.phase == "running";
        let stopped = svc.state.phase == "stopped";
        let row_id = format!("svc-{}", svc.instance_id);

        let drag = ServiceDrag {
            instance_id: svc.instance_id.clone(),
            label: svc.service_name.clone(),
        };
        let drop_id = svc.instance_id.clone();
        h_flex()
            .id(format!("{row_id}-row"))
            .on_drop(cx.listener(move |this, drag: &ServiceDrag, _, cx| {
                this.move_row(&drag.instance_id, &drop_id, cx);
            }))
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id(format!("{row_id}-drag"))
                    .flex_shrink_0()
                    .child(
                        Icon::new(IconName::GripVertical)
                            .text_color(cx.theme().muted_foreground),
                    )
                    .on_drag(drag, |drag, _, _, cx| {
                        cx.new(|_| DragGhost(drag.label.clone().into()))
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
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
                            .child({
                                let count = svc.tools.len().to_string();
                                crate::dict::t_args(
                                    cx,
                                    crate::dict::TextKey::ServiceRowToolCount,
                                    &[("count", count.as_str())],
                                )
                            }),
                    )
                    .child(
                        Button::new(format!("{row_id}-connect"))
                            .ghost()
                            .icon(IconName::Play)
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Connect))
                            .disabled(running)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(connect_addr.clone(), "connect", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-restart"))
                            .ghost()
                            .icon(IconName::RotateCw)
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Restart))
                            .disabled(stopped)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(restart_addr.clone(), "restart", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-disconnect"))
                            .ghost()
                            .icon(IconName::Square)
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Disconnect))
                            .disabled(stopped)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lifecycle(disconnect_addr.clone(), "disconnect", cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-check"))
                            .ghost()
                            .icon(IconName::Activity)
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Inspect))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.check(check_addr.clone(), cx);
                            })),
                    )
                    .child(
                        Button::new(format!("{row_id}-edit"))
                            .ghost()
                            .icon(IconName::Pencil)
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Edit))
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
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::RemoveScope))
                            .on_click({
                                let title = crate::dict::t(cx, crate::dict::TextKey::RemoveScopeTitle);
                                let body = crate::dict::t(cx, crate::dict::TextKey::RemoveScopeBody);
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
                            .tooltip(crate::dict::t(cx, crate::dict::TextKey::Details))
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
        let needle = self.search.to_lowercase();
        let rows: Vec<ServiceInstance> = self
            .order
            .iter()
            .filter_map(|id| self.services.iter().find(|svc| &svc.instance_id == id))
            .filter(|svc| {
                (needle.is_empty() || svc.service_name.to_lowercase().contains(&needle))
                    && (self.phase_filter.is_empty() || svc.state.phase == self.phase_filter)
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
                        Button::new("services-phase")
                            .outline()
                            .label(format!(
                                "phase: {}",
                                if self.phase_filter.is_empty() { "all" } else { &self.phase_filter }
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.phase_filter = match this.phase_filter.as_str() {
                                    "" => "running".to_string(),
                                    "running" => "stopped".to_string(),
                                    _ => String::new(),
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("services-sort")
                            .outline()
                            .label(crate::dict::t(cx, crate::dict::TextKey::SortLabel))
                            .on_click(cx.listener(|this, _, _, cx| this.sort_by_name(cx))),
                    )
                    .child(
                        Button::new("services-refresh")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .label(crate::dict::t(cx, crate::dict::TextKey::Refresh))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("services-add")
                            .primary()
                            .icon(IconName::Plus)
                            .label(crate::dict::t(cx, crate::dict::TextKey::AddService))
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
                                .child(crate::dict::t(cx, crate::dict::TextKey::Loading)),
                        ),
                )
            })
            .when(!self.loading && rows.is_empty(), |el| {
                el.child(
                    h_flex().justify_center().py_8().child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::dict::t(cx, crate::dict::TextKey::NoServices)),
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
                let incoming: Vec<String> = services.iter().map(|svc| svc.instance_id.clone()).collect();
                let next = reconcile_order(&this.order, &incoming);
                if next != this.order {
                    this.order = next;
                    persist_order(&this.order);
                }
                this.services = services;
                this.error = None;
            }
            Err(e) => this.error = Some(format!("{e:#}").into()),
        }
        cx.notify();
    })
    .ok();
}

#[derive(Clone)]
struct ServiceDrag {
    instance_id: String,
    label: String,
}

struct DragGhost(SharedString);

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(self.0.clone())
    }
}

/// Keep `saved` as-is. Drop ids that disappeared. Append newcomers in arrival order — no status/name sort.
fn reconcile_order(saved: &[String], incoming: &[String]) -> Vec<String> {
    let known_now: std::collections::HashSet<&str> = incoming.iter().map(String::as_str).collect();
    let mut next: Vec<String> = saved
        .iter()
        .filter(|id| known_now.contains(id.as_str()))
        .cloned()
        .collect();
    for id in incoming {
        if !next.iter().any(|kept| kept == id) {
            next.push(id.clone());
        }
    }
    next
}

fn move_id(order: &mut Vec<String>, from: &str, to: &str) {
    if from == to {
        return;
    }
    let Some(from_i) = order.iter().position(|id| id == from) else {
        return;
    };
    let Some(to_i) = order.iter().position(|id| id == to) else {
        return;
    };
    let id = order.remove(from_i);
    order.insert(to_i, id);
}

fn order_path() -> Option<std::path::PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(
            std::path::PathBuf::from(home)
                .join(".mcpstore")
                .join("gpui-service-order.json"),
        )
    }
}

fn load_order() -> Vec<String> {
    let Some(path) = order_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn persist_order(order: &[String]) {
    let Some(path) = order_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(order) {
        let _ = std::fs::write(path, text);
    }
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

#[cfg(test)]
mod tests {
    use super::{move_id, reconcile_order};

    #[test]
    fn refresh_keeps_manual_order_and_appends_newcomers() {
        let saved = vec!["b".into(), "a".into()];
        // Arrival order is whatever the server sent (here, status-ish). It must not win.
        let incoming = vec!["c".into(), "a".into(), "b".into()];
        assert_eq!(
            reconcile_order(&saved, &incoming),
            vec![String::from("b"), String::from("a"), String::from("c")]
        );
    }

    #[test]
    fn move_id_reorders_without_sorting() {
        let mut order = vec!["a".into(), "b".into(), "c".into()];
        move_id(&mut order, "c", "a");
        assert_eq!(
            order,
            vec![String::from("c"), String::from("a"), String::from("b")]
        );
    }
}
