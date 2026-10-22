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
use std::collections::HashMap;
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, Disableable as _, StyledExt as _};
use gpui_kit::component::WindowExt as _;
use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;

const LANGS: [&str; 3] = ["auto", "en", "zh"];
const HUB_QUERY: &str = "?path=/mcp&scope=store";

pub struct SettingsView {
    api: Arc<Api>,
    version: Option<SharedString>,
    language: Option<SharedString>,
    hosts: Option<HostsConfig>,
    hub: Option<SharedString>,
    hub_running: bool,
    new_host_name: Entity<InputState>,
    new_host_url: Entity<InputState>,
    editing_host: Option<String>,
    probes: HashMap<String, Option<(u64, Option<String>)>>,
    diag_enabled: bool,
    diag_runtime_enabled: bool,
    runtime_max_size: Entity<InputState>,
    runtime_max_size_now: SharedString,
    retention_days: Entity<InputState>,
    retention_days_now: SharedString,
    config_format: String,
    loading: bool,
    error: Option<SharedString>,
}

impl SettingsView {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let new_host_name = cx.new(|cx| InputState::new(window, cx).placeholder("host name"));
        let new_host_url = cx.new(|cx| InputState::new(window, cx).placeholder("http://host:port"));
        let runtime_max_size =
            cx.new(|cx| InputState::new(window, cx).placeholder("max size bytes"));
        let retention_days = cx.new(|cx| InputState::new(window, cx).placeholder("retention days"));
        let mut view = Self {
            api,
            version: None,
            language: None,
            hosts: None,
            hub: None,
            hub_running: false,
            new_host_name,
            new_host_url,
            editing_host: None,
            probes: HashMap::new(),
            diag_enabled: true,
            diag_runtime_enabled: false,
            runtime_max_size,
            runtime_max_size_now: SharedString::default(),
            retention_days,
            retention_days_now: SharedString::default(),
            config_format: "native".to_string(),
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
                    async move { api.meta().await }
                })
                .await;
            let hub = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.gateway_status(HUB_QUERY).await }
                })
                .await;
            let settings = cx
                .background_spawn(async move { api.get_settings().await })
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
                        let runtime_log = value
                            .get("diagnostics")
                            .and_then(|d| d.get("runtime_log"));
                        this.diag_enabled = value
                            .get("diagnostics")
                            .and_then(|d| d.get("enabled"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        this.diag_runtime_enabled = runtime_log
                            .and_then(|r| r.get("enabled"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        this.runtime_max_size_now = runtime_log
                            .and_then(|r| r.get("max_size_bytes"))
                            .and_then(|v| v.as_u64())
                            .map(|v| v.to_string())
                            .unwrap_or_default()
                            .into();
                        this.retention_days_now = runtime_log
                            .and_then(|r| r.get("retention_days"))
                            .and_then(|v| v.as_u64())
                            .map(|v| v.to_string())
                            .unwrap_or_default()
                            .into();
                        this.error = None;
                    }
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                if let Ok(value) = hub {
                    this.hub_running = value.get("running").and_then(|r| r.as_bool()).unwrap_or(false);
                    this.hub = Some(serde_json::to_string_pretty(&value).unwrap_or_default().into());
                }
                this.probe_hosts(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save_language(&mut self, lang: &'static str, cx: &mut Context<Self>) {
        // Flip the UI immediately, then persist.
        crate::dict::set(crate::dict::Lang::parse(Some(lang)), cx);
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let payload = serde_json::json!({ "language": lang });
            let result = cx
                .background_spawn(async move { api.update_settings(&payload).await })
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
                .background_spawn(async move { api.update_settings(&payload).await })
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

    fn persist(&mut self, payload: serde_json::Value, fail_msg: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.update_settings(&payload).await })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("{fail_msg}: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn probe_hosts(&mut self, cx: &mut Context<Self>) {
        let Some(hosts) = self.hosts.clone() else {
            return;
        };
        let api = self.api.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let results = cx
                .background_spawn(async move {
                    let mut out = Vec::new();
                    for (name, url) in hosts.entries {
                        out.push((name, api.probe_host(&url).await));
                    }
                    out
                })
                .await;
            this.update(cx, |this, cx| {
                this.probes = results.into_iter().collect();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reset_host_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_host = None;
        self.new_host_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("host name"));
        self.new_host_url =
            cx.new(|cx| InputState::new(window, cx).placeholder("http://host:port"));
        cx.notify();
    }

    fn start_edit_host(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hosts) = &self.hosts else {
            return;
        };
        let Some((_, url)) = hosts.entries.iter().find(|(n, _)| *n == name) else {
            return;
        };
        let url = url.clone();
        self.editing_host = Some(name.clone());
        self.new_host_name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("host name")
                .default_value(name)
        });
        self.new_host_url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://host:port")
                .default_value(url)
        });
        cx.notify();
    }

    fn save_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.new_host_name.read(cx).value().trim().to_string();
        let url = self.new_host_url.read(cx).value().trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.error = Some("host name and url are required".into());
            cx.notify();
            return;
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.error = Some("host url must start with http:// or https://".into());
            cx.notify();
            return;
        }
        let mut hosts = self.hosts.clone().unwrap_or_default();
        if let Some(old) = self.editing_host.take() {
            hosts.entries.retain(|(n, _)| *n != old);
        }
        if let Some(entry) = hosts.entries.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = url.clone();
        } else {
            hosts.entries.push((name.clone(), url.clone()));
        }
        let active = if hosts.entries.iter().any(|(n, _)| *n == hosts.active) {
            hosts.active.clone()
        } else {
            name.clone()
        };
        self.reset_host_inputs(window, cx);
        let payload = serde_json::json!({ "hosts": serialize_hosts(&hosts, &active) });
        self.persist(payload, "save host failed", cx);
    }

    fn delete_host(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(mut hosts) = self.hosts.clone() else {
            return;
        };
        if hosts.active == name {
            self.error = Some("cannot delete the active host".into());
            cx.notify();
            return;
        }
        if hosts.entries.len() <= 1 {
            self.error = Some("at least one host is required".into());
            cx.notify();
            return;
        }
        hosts.entries.retain(|(n, _)| *n != name);
        let payload = serde_json::json!({ "hosts": serialize_hosts(&hosts, &hosts.active) });
        self.persist(payload, "delete host failed", cx);
    }

    fn toggle_diagnostics(&mut self, which: &'static str, cx: &mut Context<Self>) {
        if which == "diag" {
            self.diag_enabled = !self.diag_enabled;
        } else {
            self.diag_runtime_enabled = !self.diag_runtime_enabled;
        }
        self.save_diagnostics(cx);
    }

    fn save_diagnostics(&mut self, cx: &mut Context<Self>) {
        let mut runtime_log = serde_json::Map::new();
        runtime_log.insert("enabled".into(), self.diag_runtime_enabled.into());
        let max_text = self.runtime_max_size.read(cx).value().trim().to_string();
        if let Ok(v) = max_text.parse::<u64>() {
            runtime_log.insert("max_size_bytes".into(), v.into());
        }
        let ret_text = self.retention_days.read(cx).value().trim().to_string();
        if let Ok(v) = ret_text.parse::<u64>() {
            runtime_log.insert("retention_days".into(), v.into());
        }
        let payload = serde_json::json!({
            "diagnostics": {
                "enabled": self.diag_enabled,
                "runtime_log": serde_json::Value::Object(runtime_log),
            }
        });
        self.persist(payload, "save diagnostics failed", cx);
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

    fn hub_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let query = if action == "stop" { "" } else { HUB_QUERY };
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.gateway_action(action, query).await })
                .await;
            this.update(cx, |this, cx| {
                this.error = result
                    .err()
                    .map(|e| format!("gateway {action} failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
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
                let edit_name = name.clone();
                let delete_name = name.clone();
                let probe = self.probes.get(name).cloned().flatten();
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
                        .when_some(probe, |el, (ms, store)| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(if ms > 0 {
                                        cx.theme().success
                                    } else {
                                        cx.theme().danger
                                    })
                                    .child(if ms > 0 {
                                        format!("{ms}ms")
                                    } else {
                                        "offline".to_string()
                                    }),
                            )
                            .when_some(store.filter(|_| ms > 0), |el, store| {
                                el.child(
                                    div().text_xs().text_color(muted).truncate().child(store),
                                )
                            })
                        })
                        .when(is_active, |el| {
                            el.child(
                                div()
                                    .rounded(cx.theme().radius)
                                    .px_2()
                                    .text_xs()
                                    .bg(cx.theme().success.opacity(0.15))
                                    .text_color(cx.theme().success)
                                    .child(crate::dict::t(cx, crate::dict::TextKey::Current)),
                            )
                        })
                        .when(!is_active, |el| {
                            el.child(
                                Button::new(format!("host-use-{name}"))
                                    .outline()
                                    .label(crate::dict::t(cx, crate::dict::TextKey::Use))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.activate_host(switch_name.clone(), cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new(format!("host-edit-{name}"))
                                .ghost()
                                .icon(IconName::Pencil)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_edit_host(edit_name.clone(), window, cx)
                                })),
                        )
                        .when(!is_active, |el| {
                            el.child(
                                Button::new(format!("host-del-{name}"))
                                    .ghost()
                                    .icon(IconName::Trash)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.delete_host(delete_name.clone(), cx)
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
                    .child(div().font_semibold().child(crate::dict::t(cx, crate::dict::TextKey::Settings)))
                    .child(div().flex_1())
                    .child(
                        Button::new("settings-theme")
                            .outline()
                            .icon(IconName::Settings)
                            .label(crate::dict::t(cx, crate::dict::TextKey::ToggleTheme))
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
                                        div().text_sm().text_color(muted).child(crate::dict::t(cx, crate::dict::TextKey::Loading)),
                                    ),
                                )
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div().text_sm().text_color(muted).child(crate::dict::t(cx, crate::dict::TextKey::Version)),
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
                                            .child(crate::dict::t(cx, crate::dict::TextKey::Language)),
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
                                            .child(crate::dict::t(cx, crate::dict::TextKey::Host)),
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
                                                Button::new("host-save")
                                                    .primary()
                                                    .icon(if self.editing_host.is_some() {
                                                        IconName::Pencil
                                                    } else {
                                                        IconName::Plus
                                                    })
                                                    .label(if self.editing_host.is_some() {
                                                        crate::dict::t(cx, crate::dict::TextKey::Save)
                                                    } else {
                                                        crate::dict::t(cx, crate::dict::TextKey::AddHost)
                                                    })
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.save_host(window, cx)
                                                    })),
                                            )
                                            .when(self.editing_host.is_some(), |el| {
                                                el.child(
                                                    Button::new("host-cancel")
                                                        .outline()
                                                        .label(crate::dict::t(cx, crate::dict::TextKey::Cancel))
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.reset_host_inputs(window, cx)
                                                            },
                                                        )),
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_sm()
                                            .text_color(muted)
                                            .child("Diagnostics"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("diag-enabled")
                                                    .outline()
                                                    .label(format!(
                                                        "enabled: {}",
                                                        if self.diag_enabled { "on" } else { "off" }
                                                    ))
                                                    .when(self.diag_enabled, |b| b.primary())
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.toggle_diagnostics("diag", cx)
                                                    })),
                                            )
                                            .child(
                                                Button::new("diag-runtime")
                                                    .outline()
                                                    .label(format!(
                                                        "runtime log: {}",
                                                        if self.diag_runtime_enabled { "on" } else { "off" }
                                                    ))
                                                    .when(self.diag_runtime_enabled, |b| b.primary())
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.toggle_diagnostics("runtime", cx)
                                                    })),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .w_40()
                                                    .child(Input::new(&self.runtime_max_size)),
                                            )
                                            .when(
                                                !self.runtime_max_size_now.is_empty(),
                                                |el| {
                                                    el.child(
                                                        div().text_xs().text_color(muted).child(
                                                            format!(
                                                                "now: {}",
                                                                self.runtime_max_size_now
                                                            ),
                                                        ),
                                                    )
                                                },
                                            )
                                            .child(
                                                div()
                                                    .w_40()
                                                    .child(Input::new(&self.retention_days)),
                                            )
                                            .when(
                                                !self.retention_days_now.is_empty(),
                                                |el| {
                                                    el.child(
                                                        div().text_xs().text_color(muted).child(
                                                            format!("now: {}", self.retention_days_now),
                                                        ),
                                                    )
                                                },
                                            )
                                            .child(
                                                Button::new("diag-save")
                                                    .outline()
                                                    .label(crate::dict::t(cx, crate::dict::TextKey::Save))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.save_diagnostics(cx)
                                                    })),
                                            ),
                                    ),
                            )
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
                                                    .child(crate::dict::t(cx, crate::dict::TextKey::Gateway)),
                                            )
                                            .when(self.hub_running, |el| {
                                                el.child(
                                                    div()
                                                        .rounded(cx.theme().radius)
                                                        .px_2()
                                                        .text_xs()
                                                        .bg(cx.theme().success.opacity(0.15))
                                                        .text_color(cx.theme().success)
                                                        .child("running"),
                                                )
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("hub-start")
                                                    .outline()
                                                    .label(crate::dict::t(cx, crate::dict::TextKey::Start))
                                                    .disabled(self.hub_running)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.hub_action("start", cx)
                                                    })),
                                            )
                                            .child(
                                                Button::new("hub-stop")
                                                    .outline()
                                                    .label(crate::dict::t(cx, crate::dict::TextKey::Stop))
                                                    .disabled(!self.hub_running)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.hub_action("stop", cx)
                                                    })),
                                            )
                                            .child(
                                                Button::new("hub-import")
                                                    .outline()
                                                    .label(crate::dict::t(cx, crate::dict::TextKey::ImportClient))
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        crate::dialogs::ImportClientDialog::open(
                                                            this.api.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    })),
                                            ),
                                    )
                                    .when_some(self.hub.clone(), |el, hub| {
                                        el.child(div().text_xs().child(hub))
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("config-view")
                                            .outline()
                                            .label(format!(
                                                "{} ({})",
                                                crate::dict::t(cx, crate::dict::TextKey::ViewConfig),
                                                self.config_format
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let api = this.api.clone();
                                                let title =
                                                    crate::dict::t(cx, crate::dict::TextKey::ViewConfig);
                                                let format = this.config_format.clone();
                                                crate::dialogs::JsonViewDialog::open_fetch(
                                                    title,
                                                    async move {
                                                        api.show_config(
                                                            &crate::api::ScopeRef::Store,
                                                            &format,
                                                        )
                                                        .await
                                                    },
                                                    window,
                                                    cx,
                                                );
                                            })),
                                    )
                                    .child(
                                        Button::new("config-format")
                                            .ghost()
                                            .icon(IconName::RotateCw)
                                            .label(format!("format: {}", self.config_format))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.config_format = match this.config_format.as_str() {
                                                    "native" => "claude",
                                                    "claude" => "codex",
                                                    _ => "native",
                                                }
                                                .to_string();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("config-reset")
                                            .ghost()
                                            .danger()
                                            .label(crate::dict::t(cx, crate::dict::TextKey::Reset))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let api = this.api.clone();
                                                window.open_alert_dialog(cx, move |alert, _, _| {
                                                    let api = api.clone();
                                                    alert
                                                        .title("Reset store config?")
                                                        .description(
                                                            "This restores the default store configuration.",
                                                        )
                                                        .on_ok(move |_, _, cx| {
                                                            let api = api.clone();
                                                            cx.spawn(
                                                                async move |cx: &mut gpui_kit::AsyncApp| {
                                                                    let result = cx
                                                                        .background_spawn(async move {
                                                                            api.reset_config(
                                                                                &crate::api::ScopeRef::Store,
                                                                            ).await
                                                                        })
                                                                        .await;
                                                                    if let Err(e) = result {
                                                                        let msg = format!("{e:#}");
                                                                        let _ = cx.update(|app| {
                                                                            if let Some(win) =
                                                                                app.windows().pop()
                                                                            {
                                                                                let _ = win.update(
                                                                                    app,
                                                                                    |_, window, cx| {
                                                                                        window.open_alert_dialog(
                                                                                            cx,
                                                                                            move |alert, _, _| {
                                                                                                alert
                                                                                                    .title("Reset failed")
                                                                                                    .description(msg.clone())
                                                                                            },
                                                                                        );
                                                                                    },
                                                                                );
                                                                            }
                                                                        });
                                                                    }
                                                                },
                                                            )
                                                            .detach();
                                                            true
                                                        })
                                                });
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}
