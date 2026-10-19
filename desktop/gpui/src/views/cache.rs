//! Cache page: backend switch + layer browser (tree from inspect/health payloads).

use crate::api::Api;
use gpui_kit::component::{button::{Button, ButtonVariants as _}, spinner::Spinner, ActiveTheme};
use gpui_kit::assets::IconName;
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use serde_json::Value;
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

const BACKENDS: [&str; 4] = ["memory", "redis", "openkeyv_memory", "openkeyv_redis"];
/// (layer id, inspect field listing key items, health field listing types)
const LAYERS: [(&str, &str, &str); 4] = [
    ("entity", "entities", "entities"),
    ("relations", "relations", "relations"),
    ("state", "states", "states"),
    ("event", "events", "events"),
];

pub struct CacheView {
    api: Arc<Api>,
    health: Option<Value>,
    inspect: Option<Value>,
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
                    async move { api.cache_health().await }
                })
                .await;
            let inspect = cx
                .background_spawn(async move { api.cache_inspect().await })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match health {
                    Ok(value) => {
                        this.health = Some(value);
                        this.error = None;
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                if let Ok(value) = inspect {
                    this.inspect = Some(value);
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
                .background_spawn(async move { api.switch_cache(backend).await })
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

/// One cache key row: (key label, value with _key/_type/_collection stripped).
type CacheEntry = (String, Value);

fn layer_rows(
    inspect: Option<&Value>,
    health: Option<&Value>,
) -> Vec<(&'static str, Vec<(String, Vec<CacheEntry>)>)> {
    let mut out = Vec::new();
    for (layer, inspect_field, health_field) in LAYERS {
        // (collection path, entries), preserving first-seen order.
        let mut collections: Vec<(String, Vec<CacheEntry>)> = Vec::new();
        if let Some(items) = inspect.and_then(|v| v.get(inspect_field)).and_then(|v| v.as_array()) {
            for raw in items {
                let Some(obj) = raw.as_object() else { continue };
                let key = obj
                    .get("_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let collection = obj
                    .get("_collection")
                    .and_then(|v| v.as_str())
                    .unwrap_or(layer)
                    .to_string();
                let mut value = obj.clone();
                value.shift_remove("_key");
                value.shift_remove("_type");
                value.shift_remove("_collection");
                let entries = match collections.iter_mut().find(|(c, _)| *c == collection) {
                    Some((_, entries)) => entries,
                    None => {
                        collections.push((collection, Vec::new()));
                        &mut collections.last_mut().unwrap().1
                    }
                };
                entries.push((key, Value::Object(value)));
            }
        } else if let Some(types) = health.and_then(|v| v.get(health_field)).and_then(|v| v.as_array())
        {
            for t in types.iter().filter_map(|t| t.as_str()) {
                collections.push((format!("{layer}:{t}"), Vec::new()));
            }
        }
        if !collections.is_empty() {
            out.push((layer, collections));
        }
    }
    out
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

        let layers = layer_rows(self.inspect.as_ref(), self.health.as_ref());
        let summary = {
            let report = self.inspect.as_ref().or(self.health.as_ref());
            report.map(|r| {
                let namespace = r.get("namespace").and_then(|v| v.as_str()).unwrap_or("default");
                let backend = r.get("backend").and_then(|v| v.as_str()).unwrap_or("unknown");
                let keys: usize = layers
                    .iter()
                    .flat_map(|(_, cols)| cols.iter().map(|(_, keys)| keys.len()))
                    .sum();
                format!("namespace: {namespace} · backend: {backend} · keys: {keys}")
            })
        };

        let mut tree = v_flex().gap_3();
        for (layer, collections) in layers {
            let mut layer_block = v_flex()
                .gap_1()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("{layer} ({} collections)", collections.len())),
                );
            for (collection, keys) in collections {
                let mut key_buttons = h_flex().gap_1().flex_wrap();
                let mut has_keys = false;
                for (key, value) in keys {
                    has_keys = true;
                    let label = if key.len() > 48 { format!("{}…", &key[..48]) } else { key };
                    key_buttons = key_buttons.child(
                        Button::new(format!("ck-{collection}-{label}"))
                            .ghost()
                            .label(label)
                            .on_click(cx.listener(move |_, _, window, cx| {
                                let title: SharedString = "cache entry".into();
                                let value = value.clone();
                                crate::dialogs::JsonViewDialog::open_fetch(
                                    title,
                                    async move { Ok::<_, anyhow::Error>(value) },
                                    window,
                                    cx,
                                );
                            })),
                    );
                }
                layer_block = layer_block.child(
                    v_flex()
                        .gap_1()
                        .pl_2()
                        .border_l_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .font_family("Monaco")
                                .text_color(cx.theme().foreground)
                                .child(collection),
                        )
                        .child(if !has_keys {
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("(no keys listed)")
                                .into_any_element()
                        } else {
                            key_buttons.into_any_element()
                        }),
                );
            }
            tree = tree.child(layer_block);
        }

        let report_text = self
            .inspect
            .as_ref()
            .or(self.health.as_ref())
            .map(|r| serde_json::to_string_pretty(r).unwrap_or_default())
            .unwrap_or_default();

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
                    .when_some(summary, |el, summary| {
                        el.child(div().text_xs().text_color(muted).child(summary))
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("cache-copy")
                            .ghost()
                            .icon(IconName::Copy)
                            .label("Copy report")
                            .disabled(report_text.is_empty())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(
                                    gpui_kit::ClipboardItem::new_string(report_text.clone()),
                                );
                            })),
                    )
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
                            .child(tree),
                    ),
            )
    }
}
