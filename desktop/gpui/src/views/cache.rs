//! Cache page: health report, layer inspection, backend switch.

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

const BACKENDS: [&str; 4] = ["memory", "redis", "openkeyv_memory", "openkeyv_redis"];

pub struct CacheView {
    api: Arc<Api>,
    health: Option<String>,
    inspect: Option<String>,
    loading: bool,
    error: Option<SharedString>,
}

impl CacheView {
    pub fn new(api: Arc<Api>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            api,
            health: None,
            inspect: None,
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
            let health = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.cache_health() }
                })
                .await;
            let inspect = cx
                .background_spawn(async move { api.cache_inspect() })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match health {
                    Ok(value) => {
                        this.health =
                            Some(serde_json::to_string_pretty(&value).unwrap_or_default());
                        this.error = None;
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                if let Ok(value) = inspect {
                    this.inspect = Some(serde_json::to_string_pretty(&value).unwrap_or_default());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn switch(&mut self, backend: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.switch_cache(backend) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("switch failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }
}

impl Render for CacheView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let mut backend_buttons = h_flex().gap_2().items_center();
        for backend in BACKENDS {
            backend_buttons = backend_buttons.child(
                Button::new(format!("cache-{backend}"))
                    .outline()
                    .label(backend)
                    .on_click(cx.listener(move |this, _, _, cx| this.switch(backend, cx))),
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
                    .child(div().font_semibold().child("Cache"))
                    .child(div().flex_1())
                    .child(
                        Button::new("cache-refresh")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .label(crate::dict::t(cx, crate::dict::Key::Refresh))
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
                            .gap_3()
                            .when(self.loading && self.health.is_none(), |el| {
                                el.child(
                                    h_flex().gap_2().items_center().child(Spinner::new()).child(
                                        div().text_sm().text_color(muted).child(crate::dict::t(cx, crate::dict::Key::Loading)),
                                    ),
                                )
                            })
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child(crate::dict::t(cx, crate::dict::Key::SwitchBackend)),
                                    )
                                    .child(backend_buttons),
                            )
                            .when_some(self.health.clone(), |el, health| {
                                el.child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .font_semibold()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(crate::dict::t(cx, crate::dict::Key::Health)),
                                        )
                                        .child(div().text_sm().child(health)),
                                )
                            })
                            .when_some(self.inspect.clone(), |el, inspect| {
                                el.child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .font_semibold()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(crate::dict::t(cx, crate::dict::Key::Inspect)),
                                        )
                                        .child(div().text_xs().child(inspect)),
                                )
                            }),
                    ),
            )
    }
}
