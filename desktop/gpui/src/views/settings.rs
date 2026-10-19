//! Settings page: meta/version, language, hosts switching (core base), theme.

use crate::api::{Api, HostsConfig};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{
    button::Button, input::{Input, InputState}, spinner::Spinner, ActiveTheme, Icon,
};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, StyledExt as _};
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

const LANGS: [&str; 3] = ["auto", "en", "zh"];

pub struct SettingsView {
    api: Arc<Api>,
    version: Option<SharedString>,
    language: Option<SharedString>,
    hosts: Option<HostsConfig>,
    new_host_name: Entity<InputState>,
    new_host_url: Entity<InputState>,
    loading: bool,
    error: Option<SharedString>,
}

impl SettingsView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let new_host_name = cx.new(|cx| InputState::new(window, cx).placeholder("host name"));
        let new_host_url = cx.new(|cx| InputState::new(window, cx).placeholder("http://host:port"));
        let mut view = Self {
            api,
            version: None,
            language: None,
            hosts: None,
            new_host_name,
            new_host_url,
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
            let meta = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.meta() }
                })
                .await;
            let settings = cx
                .background_spawn(async move { api.get_settings() })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                if let Ok(meta) = meta {
                    this.version = meta
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(SharedString::from);
                }
                match settings {
                    Ok(value) => {
                        this.language = value
                            .get("language")
                            .and_then(|l| l.as_str())
                            .map(SharedString::from);
                        this.hosts = parse_hosts(&value);
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

    fn save_language(&mut self, lang: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let payload = serde_json::json!({ "language": lang });
            let result = cx
                .background_spawn(async move { api.update_settings(&payload) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("save language failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn activate_host(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(hosts) = self.hosts.clone() else {
            return;
        };
        let Some((_, url)) = hosts.entries.iter().find(|(n, _)| *n == name) else {
            return;
        };
        let api = self.api.clone();
        let payload = serde_json::json!({ "hosts": serialize_hosts(&hosts, &name) });
        let url2 = url.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.update_settings(&payload) })
                .await;
            this.update(cx, |this, cx| {
                this.error =
                    result.err().map(|e| format!("switch host failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            // Point the client at the new core base even if the save failed.
            this.update(cx, |this, cx| {
                this.api.set_core_base(url2.clone());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn add_host(&mut self, cx: &mut Context<Self>) {
        let name = self.new_host_name.read(cx).value().trim().to_string();
        let url = self.new_host_url.read(cx).value().trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.error = Some("host name and url are required".into());
            cx.notify();
            return;
        }
        let mut hosts = self.hosts.clone().unwrap_or_default();
        if let Some(entry) = hosts.entries.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = url.clone();
        } else {
            hosts.entries.push((name.clone(), url.clone()));
        }
        let active = if hosts.active.is_empty() {
            name.clone()
        } else {
            hosts.active.clone()
        };
        let api = self.api.clone();
        let payload = serde_json::json!({ "hosts": serialize_hosts(&hosts, &active) });
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.update_settings(&payload) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("add host failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let dark = cx.theme().mode == ThemeMode::Dark;
        Theme::change(
            if dark { ThemeMode::Light } else { ThemeMode::Dark },
            None,
            cx,
        );
        cx.notify();
    }
}

fn parse_hosts(settings: &serde_json::Value) -> Option<HostsConfig> {
    let hosts = settings.get("hosts")?;
    let mut config = HostsConfig::default();
    if let Some(obj) = hosts.as_object() {
        for (key, value) in obj {
            if key == "active" {
                config.active = value.as_str().unwrap_or_default().to_string();
            } else if let Some(url) = value.get("url").and_then(|u| u.as_str()) {
                config.entries.push((key.clone(), url.to_string()));
            } else if let Some(url) = value.as_str() {
                config.entries.push((key.clone(), url.to_string()));
            }
        }
    }
    (!config.entries.is_empty()).then_some(config)
}

fn serialize_hosts(hosts: &HostsConfig, active: &str) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    out.insert("active".into(), active.into());
    for (name, url) in &hosts.entries {
        out.insert(name.clone(), serde_json::json!({ "url": url }));
    }
    serde_json::Value::Object(out)
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let mut lang_buttons = h_flex().gap_2().items_center();
        for lang in LANGS {
            lang_buttons = lang_buttons.child(
                Button::new(format!("lang-{lang}"))
                    .outline()
                    .label(lang)
                    .when(self.language.as_deref() == Some(lang), |b| b.primary())
                    .on_click(cx.listener(move |this, _, _, cx| this.save_language(lang, cx))),
            );
        }

        let mut host_rows = v_flex();
        if let Some(hosts) = &self.hosts {
            for (name, url) in &hosts.entries {
                let is_active = *name == hosts.active;
                let switch_name = name.clone();
                host_rows = host_rows.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_xs()
                                .text_color(muted)
                                .truncate()
                                .child(url.clone()),
                        )
                        .when(is_active, |el| {
                            el.child(
                                div()
                                    .rounded(cx.theme().radius)
                                    .px_2()
                                    .text_xs()
                                    .bg(cx.theme().success.opacity(0.15))
                                    .text_color(cx.theme().success)
                                    .child(crate::dict::t(cx, crate::dict::Key::Active)),
                            )
                        })
                        .when(!is_active, |el| {
                            el.child(
                                Button::new(format!("host-use-{name}"))
                                    .outline()
                                    .label(crate::dict::t(cx, crate::dict::Key::Use))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.activate_host(switch_name.clone(), cx)
                                    })),
                            )
                        }),
                );
            }
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
                    .child(div().font_semibold().child(crate::dict::t(cx, crate::dict::Key::Settings)))
                    .child(div().flex_1())
                    .child(
                        Button::new("settings-theme")
                            .outline()
                            .icon(IconName::Settings)
                            .label(crate::dict::t(cx, crate::dict::Key::ToggleTheme))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                    )
                    .child(
                        Button::new("settings-refresh")
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
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_4()
                            .when(self.loading, |el| {
                                el.child(
                                    h_flex().gap_2().items_center().child(Spinner::new()).child(
                                        div().text_sm().text_color(muted).child(crate::dict::t(cx, crate::dict::Key::Loading)),
                                    ),
                                )
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div().text_sm().text_color(muted).child(crate::dict::t(cx, crate::dict::Key::Version)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .child(self.version.clone().unwrap_or_else(|| "—".into())),
                                    )
                                    .child(Icon::new(IconName::Info).text_color(muted))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(format!("app: {}", self.api.app_base())),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child(crate::dict::t(cx, crate::dict::Key::Language)),
                                    )
                                    .child(lang_buttons),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child(crate::dict::t(cx, crate::dict::Key::Hosts)),
                                    )
                                    .child(host_rows)
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .pt_2()
                                            .child(
                                                div().w_40().child(Input::new(&self.new_host_name)),
                                            )
                                            .child(
                                                div().flex_1().child(Input::new(&self.new_host_url)),
                                            )
                                            .child(
                                                Button::new("host-add")
                                                    .primary()
                                                    .icon(IconName::Plus)
                                                    .label(crate::dict::t(cx, crate::dict::Key::AddHost))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.add_host(cx)
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
