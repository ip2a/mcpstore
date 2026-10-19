//! Cache page: health report from `/cache/health`.

use crate::api::Api;
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

pub struct CacheView {
    api: Arc<Api>,
    report: Option<String>,
    loading: bool,
    error: Option<SharedString>,
}

impl CacheView {
    pub fn new(api: Arc<Api>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            api,
            report: None,
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
            let result = cx.background_spawn(async move { api.cache_health() }).await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(value) => {
                        this.report = Some(
                            serde_json::to_string_pretty(&value).unwrap_or_default(),
                        );
                        this.error = None;
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

impl Render for CacheView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(div().font_semibold().child("Cache health"))
                    .child(div().flex_1())
                    .child(
                        Button::new("cache-refresh")
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
                    .id("cache-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_2()
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
                            .children(self.report.clone().map(|report| {
                                div().text_sm().child(report)
                            })),
                    ),
            )
    }
}
