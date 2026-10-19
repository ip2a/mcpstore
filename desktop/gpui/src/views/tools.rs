//! Aggregated tool registry across all services (scope=root).

use crate::api::{Api, ScopeView, ServiceAddress, ToolInfo};
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    spinner::Spinner,
    ActiveTheme, Icon, IconName,
};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, StyledExt as _};
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

pub struct ToolsView {
    api: Arc<Api>,
    query: Entity<InputState>,
    search: String,
    tools: Vec<(ServiceAddress, ToolInfo)>,
    loading: bool,
    error: Option<SharedString>,
    _search_sub: gpui_kit::Subscription,
}

impl ToolsView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search tools…"));
        let search_sub = cx.subscribe_in(&query, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.search = state.read(cx).value().to_string();
                cx.notify();
            }
        });
        let mut view = Self {
            api,
            query,
            search: String::new(),
            tools: Vec::new(),
            loading: true,
            error: None,
            _search_sub: search_sub,
        };
        view.refresh(cx);
        view
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let services = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.list_services(&ScopeView::Root) }
                })
                .await;
            let Err(e) = services else {
                // Fetch tools per service sequentially in one background task.
                let result = cx
                    .background_spawn(async move {
                        let mut out = Vec::new();
                        for svc in services.unwrap_or_default() {
                            let addr = ServiceAddress {
                                service_name: svc.service_name.clone(),
                                scope: svc.scope.clone(),
                            };
                            if let Ok(tools) = api.list_tools(&addr) {
                                for tool in tools {
                                    out.push((addr.clone(), tool));
                                }
                            }
                        }
                        Ok::<_, anyhow::Error>(out)
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.loading = false;
                    match result {
                        Ok(tools) => {
                            this.tools = tools;
                            this.error = None;
                        }
                        Err(e) => this.error = Some(format!("{e:#}").into()),
                    }
                    cx.notify();
                })
                .ok();
                return Ok::<_, anyhow::Error>(());
            };
            let message = format!("{e:#}");
            this.update(cx, |this, cx| {
                this.loading = false;
                this.error = Some(message.into());
                cx.notify();
            })
            .ok();
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}

impl Render for ToolsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.query.clone();
        let q = self.search.to_lowercase();
        let rows: Vec<&(ServiceAddress, ToolInfo)> = self
            .tools
            .iter()
            .filter(|(_, tool)| {
                q.is_empty()
                    || tool.name.to_lowercase().contains(&q)
                    || tool.description.to_lowercase().contains(&q)
            })
            .collect();

        let mut list = v_flex();
        for (addr, tool) in rows {
            let id = format!(
                "tool-{}-{}",
                urlencoding::encode(&addr.service_name),
                urlencoding::encode(&tool.name)
            );
            list = list.child(
                v_flex()
                    .id(id)
                    .gap_0p5()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_medium()
                                    .text_sm()
                                    .child(tool.title.clone().unwrap_or_else(|| tool.name.clone())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(addr.service_name.clone()),
                            ),
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
                    .child(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .child(div().flex_1().min_w_0().child(Input::new(&query)))
                    .child(
                        Button::new("tools-refresh")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .label("Refresh")
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
            .when(self.loading && self.tools.is_empty(), |el| {
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
                                .child("Loading tools…"),
                        ),
                )
            })
            .child(
                div()
                    .id("tools-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(list),
            )
    }
}
