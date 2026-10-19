//! One service's detail page: overview / tools / resources / prompts / auth tabs.

use crate::api::{Api, AuthStatusView, PromptInfo, ResourceInfo, ResourceTemplateInfo, ServiceAddress, ServiceInstance, ToolInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::tab::TabBar;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputState},
    spinner::Spinner,
    ActiveTheme, Icon,
};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, IntoElement, ParentElement as _, Render,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Tools,
    Resources,
    Prompts,
    Overrides,
    Auth,
}

const OVERRIDE_KINDS: [&str; 4] = ["tool", "prompt", "resource", "resource_template"];

pub struct InstanceView {
    addr: ServiceAddress,
    api: Arc<Api>,
    tab: Tab,
    instance: Option<ServiceInstance>,
    tools: Vec<ToolInfo>,
    resources: Vec<ResourceInfo>,
    templates: Vec<ResourceTemplateInfo>,
    prompts: Vec<PromptInfo>,
    auth: Option<AuthStatusView>,
    overrides: Vec<Option<serde_json::Value>>,
    ov_kind: usize,
    ov_key: Entity<InputState>,
    ov_name: Entity<InputState>,
    ov_desc: Entity<InputState>,
    loading: bool,
    error: Option<SharedString>,
}

impl EventEmitter<BackToServices> for InstanceView {}

impl InstanceView {
    pub fn new(addr: ServiceAddress, api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ov_key = cx.new(|cx| InputState::new(window, cx).placeholder("tool / prompt / uri"));
        let ov_name = cx.new(|cx| InputState::new(window, cx).placeholder("display name"));
        let ov_desc = cx.new(|cx| InputState::new(window, cx).placeholder("description"));
        let mut view = Self {
            addr,
            api,
            tab: Tab::Overview,
            instance: None,
            tools: Vec::new(),
            resources: Vec::new(),
            templates: Vec::new(),
            prompts: Vec::new(),
            auth: None,
            overrides: Vec::new(),
            ov_kind: 0,
            ov_key,
            ov_name,
            ov_desc,
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
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.list_tools(&addr) }
                })
                .await;
            let resources = cx
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.list_resources(&addr) }
                })
                .await;
            let templates = cx
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.list_resource_templates(&addr) }
                })
                .await;
            let prompts = cx
                .background_spawn({
                    let api = api.clone();
                    let addr = addr.clone();
                    async move { api.list_prompts(&addr) }
                })
                .await;
            let auth = cx
                .background_spawn({
                    let api = api.clone();
                    async move { api.auth_status(&addr) }
                })
                .await;
            let overrides = cx
                .background_spawn({
                    let api = api.clone();
                    async move {
                        let mut out = Vec::new();
                        for kind in OVERRIDE_KINDS {
                            out.push(api.list_overrides(kind).ok());
                        }
                        out
                    }
                })
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
                if let Ok(resources) = resources {
                    this.resources = resources;
                }
                if let Ok(templates) = templates {
                    this.templates = templates;
                }
                if let Ok(prompts) = prompts {
                    this.prompts = prompts;
                }
                if let Ok(auth) = auth {
                    this.auth = Some(auth);
                }
                this.overrides = overrides;
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

    fn auth_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let addr = self.addr.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.auth_action(&addr, action) })
                .await;
            if let Ok(op) = &result {
                if let Some(url) = op.authorization.clone().map(|a| a.authorization_url) {
                    cx.update(|cx| cx.open_url(&url));
                }
            }
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("{action} failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn clear_policy(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let addr = self.addr.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.clear_tool_policy(&addr) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result.err().map(|e| format!("clear policy failed: {e:#}").into());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn add_override(&mut self, cx: &mut Context<Self>) {
        let kind = OVERRIDE_KINDS[self.ov_kind];
        let key = self.ov_key.read(cx).value().trim().to_string();
        let name = self.ov_name.read(cx).value().trim().to_string();
        let desc = self.ov_desc.read(cx).value().trim().to_string();
        if key.is_empty() {
            self.error = Some("override key is required".into());
            cx.notify();
            return;
        }
        let mut patch = serde_json::Map::new();
        patch.insert("enabled".into(), serde_json::Value::Bool(true));
        if !name.is_empty() {
            patch.insert("display_name".into(), name.into());
        }
        if !desc.is_empty() {
            patch.insert("description".into(), desc.into());
        }
        let api = self.api.clone();
        let svc = self.addr.service_name.clone();
        let patch = serde_json::Value::Object(patch);
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.set_override(kind, &svc, &key, &patch) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result
                    .err()
                    .map(|e| format!("set override failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn toggle_override(&mut self, kind: &'static str, rule: serde_json::Value, cx: &mut Context<Self>) {
        let key = override_rule_key(kind, &rule);
        if key.is_empty() {
            return;
        }
        // Preserve editable fields, flip enabled.
        let mut patch = serde_json::Map::new();
        for field in [
            "display_name",
            "description",
            "meta",
            "annotations",
            "tags",
            "arguments",
            "safety_policy",
            "mime_type",
        ] {
            if let Some(v) = rule.get(field) {
                patch.insert(field.into(), v.clone());
            }
        }
        let enabled = rule.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);
        patch.insert("enabled".into(), (!enabled).into());
        let api = self.api.clone();
        let svc = self.addr.service_name.clone();
        let patch = serde_json::Value::Object(patch);
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.set_override(kind, &svc, &key, &patch) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result
                    .err()
                    .map(|e| format!("toggle override failed: {e:#}").into());
                cx.notify();
            })
            .ok();
            this.update(cx, |this, cx| this.refresh(cx)).ok();
        })
        .detach();
    }

    fn delete_override(&mut self, kind: &'static str, rule: serde_json::Value, cx: &mut Context<Self>) {
        let key = override_rule_key(kind, &rule);
        if key.is_empty() {
            return;
        }
        let api = self.api.clone();
        let svc = self.addr.service_name.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move { api.delete_override(kind, &svc, &key) })
                .await;
            this.update(cx, |this, cx| {
                this.error = result
                    .err()
                    .map(|e| format!("delete override failed: {e:#}").into());
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
        let tab = self.tab;

        // Tab bar
        let tab_names = [
            crate::dict::t(cx, crate::dict::Key::Overview),
            crate::dict::t(cx, crate::dict::Key::Tools),
            crate::dict::t(cx, crate::dict::Key::Resources),
            crate::dict::t(cx, crate::dict::Key::Prompts),
            crate::dict::t(cx, crate::dict::Key::Overrides),
            crate::dict::t(cx, crate::dict::Key::Auth),
        ];
        let tab_bar = TabBar::new("instance-tabs")
            .children(tab_names)
            .selected_index(match tab {
                Tab::Overview => 0,
                Tab::Tools => 1,
                Tab::Resources => 2,
                Tab::Prompts => 3,
                Tab::Overrides => 4,
                Tab::Auth => 5,
            })
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.tab = match ix {
                    0 => Tab::Overview,
                    1 => Tab::Tools,
                    2 => Tab::Resources,
                    3 => Tab::Prompts,
                    4 => Tab::Overrides,
                    _ => Tab::Auth,
                };
                cx.notify();
            }));

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
                .child(Self::render_field("Tools sync", svc.state.tools.status.clone(), muted, fg, border))
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

        let mut tool_rows = v_flex();
        for tool in &self.tools {
            tool_rows = tool_rows.child(
                v_flex()
                    .gap_0p5()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .font_medium()
                                    .text_sm()
                                    .child(tool.title.clone().unwrap_or_else(|| tool.name.clone())),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new(format!("tool-run-{}", urlencoding::encode(&tool.name)))
                                    .outline()
                                    .label(crate::dict::t(cx, crate::dict::Key::Run))
                                    .on_click({
                                        let api = self.api.clone();
                                        let addr = self.addr.clone();
                                        let tool = tool.clone();
                                        move |_, window, cx| {
                                            crate::dialogs::ToolRunDialog::open(
                                                addr.clone(),
                                                tool.clone(),
                                                api.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    }),
                            )
                            .child(
                                Button::new(format!("tool-policy-{}", urlencoding::encode(&tool.name)))
                                    .ghost()
                                    .icon(IconName::Settings)
                                    .tooltip("Tool policy")
                                    .on_click({
                                        let api = self.api.clone();
                                        let addr = self.addr.clone();
                                        move |_, window, cx| {
                                            crate::dialogs::ToolPolicyDialog::open(
                                                addr.clone(),
                                                api.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    }),
                            ),
                    )
                    .when_some(
                        (!tool.description.is_empty()).then(|| tool.description.clone()),
                        |el, desc| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .truncate()
                                    .child(desc),
                            )
                        },
                    ),
            );
        }
        tool_rows = tool_rows.child(
            h_flex().gap_2().pt_2().child(
                Button::new("policy-clear")
                    .ghost()
                    .danger()
                    .label(crate::dict::t(cx, crate::dict::Key::Clear))
                    .disabled(!running)
                    .on_click(cx.listener(|this, _, _, cx| this.clear_policy(cx))),
            ),
        );

        let mut resource_rows = v_flex();
        for res in &self.resources {
            resource_rows = resource_rows.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .child(div().text_sm().truncate().child(res.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .truncate()
                                    .child(res.uri.clone()),
                            ),
                    )
                    .child(
                        Button::new(format!("res-read-{}", urlencoding::encode(&res.uri)))
                            .outline()
                            .label(crate::dict::t(cx, crate::dict::Key::Read))
                            .on_click({
                                let api = self.api.clone();
                                let addr = self.addr.clone();
                                let uri = res.uri.clone();
                                move |_, window, cx| {
                                    crate::dialogs::ResourceReadDialog::open(
                                        addr.clone(),
                                        api.clone(),
                                        uri.clone(),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    ),
            );
        }

        let mut template_rows = v_flex();
        for tpl in &self.templates {
            template_rows = template_rows.child(
                v_flex()
                    .gap_0p5()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(div().text_sm().truncate().child(tpl.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child(tpl.uri_template.clone()),
                    )
                    .when_some(tpl.description.clone(), |el, desc| {
                        el.child(div().text_xs().text_color(muted).truncate().child(desc))
                    }),
            );
        }

        let mut prompt_rows = v_flex();
        for prompt in &self.prompts {
            prompt_rows = prompt_rows.child(
                v_flex()
                    .gap_0p5()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_sm()
                            .child(prompt.title.clone().unwrap_or_else(|| prompt.name.clone())),
                    )
                    .when_some(prompt.description.clone(), |el, desc| {
                        el.child(div().text_xs().text_color(muted).truncate().child(desc))
                    }),
            );
        }

        let auth_status = self.auth.as_ref().map(|a| a.status.clone()).unwrap_or_default();
        let mut auth_rows = v_flex();
        auth_rows = auth_rows
            .child(Self::render_field("Auth status", auth_status.clone(), muted, fg, border));
        if let Some(auth) = &self.auth {
            if let Some(required) = &auth.required_scope {
                auth_rows = auth_rows.child(Self::render_field(
                    "Required scope",
                    required.clone(),
                    muted,
                    fg,
                    border,
                ));
            }
        }
        let auth_actions = h_flex()
            .gap_2()
            .pt_3()
            .child(
                Button::new("auth-start")
                    .outline()
                    .icon(IconName::ExternalLink)
                    .label(crate::dict::t(cx, crate::dict::Key::StartAuth))
                    .disabled(running == false && stopped == false)
                    .on_click(cx.listener(|this, _, _, cx| this.auth_action("start", cx))),
            )
            .child(
                Button::new("auth-refresh")
                    .outline()
                    .label(crate::dict::t(cx, crate::dict::Key::RefreshToken))
                    .on_click(cx.listener(|this, _, _, cx| this.auth_action("refresh", cx))),
            )
            .child(
                Button::new("auth-logout")
                    .outline()
                    .label(crate::dict::t(cx, crate::dict::Key::Logout))
                    .on_click(cx.listener(|this, _, _, cx| this.auth_action("logout", cx))),
            );

        let content = match tab {
            Tab::Overview => fields.into_any_element(),
            Tab::Tools => tool_rows.into_any_element(),
            Tab::Resources => v_flex()
                .child(resource_rows)
                .when(!self.templates.is_empty(), |el| {
                    el.child(
                        div()
                            .mt_2()
                            .font_semibold()
                            .text_sm()
                            .text_color(muted)
                            .child(crate::dict::t(cx, crate::dict::Key::Templates)),
                    )
                    .child(template_rows)
                })
                .into_any_element(),
            Tab::Prompts => prompt_rows.into_any_element(),
            Tab::Overrides => self.render_overrides(cx).into_any_element(),
            Tab::Auth => v_flex()
                .child(auth_rows)
                .child(auth_actions)
                .into_any_element(),
        };

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
                    .border_color(border)
                    .child(
                        Button::new("instance-back")
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .on_click(cx.listener(|_this, _, _, cx| {
                                cx.emit(BackToServices);
                            })),
                    )
                    .child(
                        Icon::new(IconName::Server).text_color(cx.theme().muted_foreground),
                    )
                    .child(div().font_semibold().child(format!("Service · {name}")))
                    .child(div().flex_1())
                    .child(
                        Button::new("instance-connect")
                            .outline()
                            .icon(IconName::Play)
                            .label(crate::dict::t(cx, crate::dict::Key::Connect))
                            .disabled(running)
                            .on_click(cx.listener(|this, _, _, cx| this.lifecycle("connect", cx))),
                    )
                    .child(
                        Button::new("instance-restart")
                            .outline()
                            .icon(IconName::RotateCw)
                            .label(crate::dict::t(cx, crate::dict::Key::Restart))
                            .disabled(stopped)
                            .on_click(cx.listener(|this, _, _, cx| this.lifecycle("restart", cx))),
                    )
                    .child(
                        Button::new("instance-disconnect")
                            .outline()
                            .icon(IconName::Square)
                            .label(crate::dict::t(cx, crate::dict::Key::Disconnect))
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
            .child(
                h_flex().px_3().py_1().border_b_1().border_color(border).child(tab_bar),
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
                                                .child(crate::dict::t(cx, crate::dict::Key::Loading)),
                                        ),
                                )
                            })
                            .child(content),
                    ),
            )
    }
}

impl InstanceView {
    fn render_overrides(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let svc_name = self.addr.service_name.clone();

        // Add form: kind picker + key + display name + description.
        let mut kind_buttons = h_flex().gap_2().items_center();
        for (i, kind) in OVERRIDE_KINDS.iter().enumerate() {
            kind_buttons = kind_buttons.child(
                Button::new(format!("ov-kind-{kind}"))
                    .outline()
                    .label(*kind)
                    .when(self.ov_kind == i, |b| b.primary())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ov_kind = i;
                        cx.notify();
                    })),
            );
        }
        let add_form = v_flex()
            .gap_2()
            .child(kind_buttons)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().w_56().child(Input::new(&self.ov_key)))
                    .child(div().w_48().child(Input::new(&self.ov_name)))
                    .child(div().flex_1().child(Input::new(&self.ov_desc)))
                    .child(
                        Button::new("ov-add")
                            .primary()
                            .label(crate::dict::t(cx, crate::dict::Key::AddOverride))
                            .on_click(cx.listener(|this, _, _, cx| this.add_override(cx))),
                    ),
            );

        // Per-kind sections listing this service's rules.
        let mut sections = v_flex().gap_3();
        for (i, kind) in OVERRIDE_KINDS.iter().enumerate() {
            let mut rows = v_flex();
            if let Some(Some(list)) = self.overrides.get(i) {
                if let Some(rules) = list.get("overrides").and_then(|o| o.as_array()) {
                    for rule in rules
                        .iter()
                        .filter(|r| r.get("service_name").and_then(|s| s.as_str()) == Some(svc_name.as_str()))
                    {
                        let key = override_rule_key(kind, rule);
                        let display = rule
                            .get("display_name")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let desc = rule
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let enabled = rule
                            .get("enabled")
                            .and_then(|e| e.as_bool())
                            .unwrap_or(true);
                        let rule_clone = rule.clone();
                        let rule_clone2 = rule.clone();
                        rows = rows.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .py_1p5()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .text_sm()
                                        .truncate()
                                        .child(if display.is_empty() {
                                            key.clone()
                                        } else {
                                            format!("{key} → {display}")
                                        }),
                                )
                                .when(!desc.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_xs()
                                            .text_color(muted)
                                            .truncate()
                                            .child(desc),
                                    )
                                })
                                .child(
                                    div()
                                        .rounded(cx.theme().radius)
                                        .px_2()
                                        .text_xs()
                                        .when(enabled, |el| {
                                            el.bg(cx.theme().success.opacity(0.15))
                                                .text_color(cx.theme().success)
                                                .child("on")
                                        })
                                        .when(!enabled, |el| {
                                            el.bg(cx.theme().muted.opacity(0.4))
                                                .text_color(muted)
                                                .child("off")
                                        }),
                                )
                                .child(
                                    Button::new(format!("ov-toggle-{i}-{key}"))
                                        .ghost()
                                        .label(if enabled {
                                            crate::dict::t(cx, crate::dict::Key::Disable)
                                        } else {
                                            crate::dict::t(cx, crate::dict::Key::Enable)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_override(kind, rule_clone.clone(), cx)
                                        })),
                                )
                                .child(
                                    Button::new(format!("ov-del-{i}-{key}"))
                                        .ghost()
                                        .danger()
                                        .icon(IconName::Trash)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.delete_override(kind, rule_clone2.clone(), cx)
                                        })),
                                ),
                        );
                    }
                }
            }
            sections = sections.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .font_semibold()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("{kind}_overrides")),
                    )
                    .child(rows),
            );
        }

        v_flex()
            .gap_4()
            .child(add_form)
            .child(sections)
    }
}

/// Identity of an override rule: tool/prompt use name segments, resources use query keys.
fn override_rule_key(kind: &str, rule: &serde_json::Value) -> String {
    let field = match kind {
        "tool" => "tool_name",
        "prompt" => "prompt_name",
        "resource" => "uri",
        _ => "uri_template",
    };
    rule.get(field)
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string()
}
