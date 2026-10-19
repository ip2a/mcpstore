//! Scopes page: registry of root/store/agent scopes plus the agent list.

use crate::api::{Api, ScopeSummary};
use gpui_kit::component::{button::Button, spinner::Spinner, ActiveTheme};
use gpui_kit::assets::IconName;
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, StyledExt as _};
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

pub struct ScopesView {
    api: Arc<Api>,
    scopes: Vec<ScopeSummary>,
    agents: Vec<(String, usize)>,
    loading: bool,
    error: Option<SharedString>,
}

impl ScopesView {
    pub fn new(api: Arc<Api>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            api,
            scopes: Vec::new(),
            agents: Vec::new(),
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
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let scopes = cx.background_spawn({
                let api = api.clone();
                async move { api.list_scopes() }
            })
            .await;
            let agents = cx.background_spawn(async move { api.list_agents() }).await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match (scopes, agents) {
                    (Ok(scopes), Ok(agents)) => {
                        this.scopes = scopes;
                        this.agents = agents
                            .into_iter()
                            .map(|a| (a.agent_id, a.instance_ids.len()))
                            .collect();
                        this.error = None;
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        this.error = Some(format!("{e:#}").into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ScopesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut scope_rows = v_flex();
        for summary in &self.scopes {
            let label = match &summary.scope {
                crate::api::ScopeView::Root => "root".to_string(),
                crate::api::ScopeView::Store => "store".to_string(),
                crate::api::ScopeView::Agent { agent_id } => format!("agent:{agent_id}"),
            };
            scope_rows = scope_rows.child(
                h_flex()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_sm().child(label))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} services", summary.service_count)),
                    ),
            );
        }

        let mut agent_rows = v_flex();
        for (agent_id, count) in &self.agents {
            agent_rows = agent_rows.child(
                h_flex()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_sm().child(agent_id.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{count} instances")),
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
                        div()
                            .font_semibold()
                            .child("Scopes"),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("scopes-refresh")
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
            .child(
                div()
                    .id("scopes-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_3()
                            .when(self.loading, |el| {
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
                            .child(
                                div()
                                    .font_semibold()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Scope registry"),
                            )
                            .child(scope_rows)
                            .child(
                                div()
                                    .mt_2()
                                    .font_semibold()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Agents"),
                            )
                            .child(agent_rows),
                    ),
            )
    }
}
