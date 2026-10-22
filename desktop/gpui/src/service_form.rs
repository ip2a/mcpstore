//! Add / edit service dialog — layout and interaction mirrored from the React
//! `AddServiceDialog` / `AddServiceForm` / `AddServicePlaygroundAside`.

use crate::api::{
    Api, HANDSHAKE_MODES, RESTART_POLICIES, STARTUP_POLICIES, ScopeRef, ServiceConfigFormat,
    ServiceFormDraft, ServiceInstance, build_add_service_cli,
    parse_imported_service_config_format, serialize_service_config,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex, Disableable as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::collapsible::Collapsible;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::TabBar;
use gpui_kit::component::{
    WindowExt as _,
    button::Button,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    ActiveTheme, Icon, IndexPath, Sizable as _, Size,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, px, relative, rgb,
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub enum FormMode {
    Add,
    Edit {
        name: String,
        scope: ScopeRef,
        config: Value,
    },
}

/// Add / edit service form dialog. Edit mode prefills and PUTs the scope descriptor.
pub struct ServiceFormDialog {
    api: Arc<Api>,
    mode: FormMode,
    name: Entity<InputState>,
    /// `false` = stdio, `true` = streamable-http (UI label "http").
    transport_http: bool,
    command: Entity<InputState>,
    args_rows: Vec<LineRow>,
    url: Entity<InputState>,
    description: Entity<InputState>,
    working_dir: Entity<InputState>,
    env_rows: Vec<PairRow>,
    header_rows: Vec<HeaderRow>,
    next_row: u64,
    oauth_enabled: bool,
    oauth_client_id: Entity<InputState>,
    oauth_client_metadata_url: Entity<InputState>,
    handshake: Entity<SelectState<SearchableVec<&'static str>>>,
    startup: Entity<SelectState<SearchableVec<&'static str>>>,
    restart: Entity<SelectState<SearchableVec<&'static str>>>,
    scope_agent: bool,
    agent_id: Entity<InputState>,
    /// Readonly identity shown in edit mode (React name + scope row, transport field).
    scope_label: Entity<InputState>,
    transport_text: Entity<InputState>,
    agents: Vec<String>,
    open_env: bool,
    open_cwd: bool,
    open_headers: bool,
    open_oauth: bool,
    open_desc: bool,
    open_handshake: bool,
    open_scope: bool,
    open_lifecycle: bool,
    preview_format: ServiceConfigFormat,
    preview: Entity<TextareaState>,
    preview_error: Option<SharedString>,
    preview_dirty: bool,
    /// Set while the form writes the playground, so that write does not parse back.
    writing_preview: bool,
    error: Option<SharedString>,
    done: bool,
    running: bool,
    _subs: Vec<Subscription>,
}

struct LineRow {
    id: u64,
    input: Entity<InputState>,
    _sub: Subscription,
}

struct PairRow {
    id: u64,
    key: Entity<InputState>,
    value: Entity<InputState>,
    _subs: Vec<Subscription>,
}

struct HeaderRow {
    id: u64,
    key: Entity<InputState>,
    value: Entity<TextareaState>,
    _subs: Vec<Subscription>,
}

/// React `stringListFromText`: keep blank lines; an empty string is one blank row.
fn lines_from_text(text: &str) -> Vec<String> {
    if text.is_empty() {
        vec![String::new()]
    } else {
        text.split('\n').map(str::to_string).collect()
    }
}

fn lines_to_text(lines: &[String]) -> String {
    lines.join("\n")
}

/// React `keyValuePairsFromText`, plus one blank row when nothing is left.
fn pairs_from_text(text: &str) -> Vec<(String, String)> {
    if text.trim().is_empty() {
        return vec![(String::new(), String::new())];
    }
    let mut out = Vec::new();
    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed.find('=') {
            Some(index) if index > 0 => {
                out.push((trimmed[..index].trim().to_string(), trimmed[index + 1..].to_string()));
            }
            _ => out.push((trimmed.to_string(), String::new())),
        }
    }
    if out.is_empty() {
        out.push((String::new(), String::new()));
    }
    out
}

fn pairs_to_text(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .filter(|(key, value)| !key.trim().is_empty() || !value.trim().is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn tr(cx: &gpui_kit::App, key: crate::dict::TextKey) -> &'static str {
    crate::dict::t(cx, key)
}

/// Right playground column. Dialog width is the 400px form column plus this,
/// plus body padding (`px_5` × 2) and the column gap (`gap_5`): 4px × (5+5+5).
const ASIDE_W: f32 = 320.;
const HANDSHAKE_UI: [&str; 3] = ["auto", "discover", "initialize"];

fn option_index(options: &[&str], value: &str) -> IndexPath {
    IndexPath::new(options.iter().position(|&item| item == value).unwrap_or(0))
}

fn select_value(
    state: &Entity<SelectState<SearchableVec<&'static str>>>,
    fallback: &'static str,
    cx: &gpui_kit::App,
) -> String {
    state
        .read(cx)
        .selected_value()
        .copied()
        .unwrap_or(fallback)
        .to_string()
}

fn new_input(
    placeholder: &str,
    value: &str,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn new_area(
    placeholder: &str,
    value: &str,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) -> Entity<TextareaState> {
    cx.new(|cx| {
        TextareaState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn new_select(
    options: &[&'static str],
    value: &str,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) -> Entity<SelectState<SearchableVec<&'static str>>> {
    let index = option_index(options, value);
    cx.new(|cx| SelectState::new(SearchableVec::new(options.to_vec()), Some(index), window, cx))
}

fn bind_change<S>(
    state: &Entity<S>,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) -> Subscription
where
    S: EventEmitter<InputEvent> + 'static,
{
    cx.subscribe_in(state, window, |this, _, event, window, cx| {
        if matches!(event, InputEvent::Change) {
            this.sync_preview_from_form(window, cx);
        }
    })
}

fn watch_text<S>(
    subs: &mut Vec<Subscription>,
    state: &Entity<S>,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) where
    S: EventEmitter<InputEvent> + 'static,
{
    subs.push(cx.subscribe_in(state, window, |this, _, event, window, cx| {
        if matches!(event, InputEvent::Change) {
            this.sync_preview_from_form(window, cx);
        }
    }));
}

fn watch_select(
    subs: &mut Vec<Subscription>,
    state: &Entity<SelectState<SearchableVec<&'static str>>>,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) {
    subs.push(cx.subscribe_in(state, window, |this, _, event, window, cx| {
        if matches!(event, SelectEvent::Confirm(_)) {
            this.sync_preview_from_form(window, cx);
        }
    }));
}

fn set_input(
    state: &Entity<InputState>,
    value: String,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) {
    state.update(cx, |state, cx| state.set_value(value, window, cx));
}

fn set_area(
    state: &Entity<TextareaState>,
    value: String,
    window: &mut Window,
    cx: &mut Context<ServiceFormDialog>,
) {
    state.update(cx, |state, cx| state.set_value(value, window, cx));
}

impl ServiceFormDialog {
    pub fn open_add(api: Arc<Api>, window: &mut Window, cx: &mut gpui_kit::App) {
        Self::open(api, FormMode::Add, window, cx);
    }

    pub fn open_edit(
        api: Arc<Api>,
        name: String,
        scope: ScopeRef,
        svc: &ServiceInstance,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let mut config = svc.effective_config.clone();
        if config.get("transport").is_none() {
            config["transport"] = svc.transport.clone().into();
        }
        if config.get("command").is_none() {
            if let Some(command) = svc.command.clone() {
                config["command"] = command.into();
            }
        }
        if config.get("url").is_none() {
            if let Some(url) = svc.url.clone() {
                config["url"] = url.into();
            }
        }
        Self::open(
            api,
            FormMode::Edit {
                name,
                scope,
                config,
            },
            window,
            cx,
        );
    }

    fn open(api: Arc<Api>, mode: FormMode, window: &mut Window, cx: &mut gpui_kit::App) {
        let name_val = match &mode {
            FormMode::Add => String::new(),
            FormMode::Edit { name, .. } => name.clone(),
        };
        let draft = match &mode {
            FormMode::Add => ServiceFormDraft::default(),
            FormMode::Edit { config, .. } => ServiceFormDraft::from_config(config),
        };
        let transport_http = draft.transport != "stdio";
        let preview_text = serialize_service_config(&draft, ServiceConfigFormat::Json);

        let entity = cx.new(|cx| {
            let name = new_input("github", &name_val, window, cx);
            let command = new_input("npx", &draft.command, window, cx);
            let url = new_input("https://example.com/mcp", &draft.url, window, cx);
            let description = new_input(
                tr(cx, crate::dict::TextKey::OptionalDescription),
                &draft.description,
                window,
                cx,
            );
            let working_dir = new_input(tr(cx, crate::dict::TextKey::Optional), &draft.working_dir, window, cx);
            let oauth_client_id = new_input(
                tr(cx, crate::dict::TextKey::OauthClientIdHint),
                &draft.oauth_client_id,
                window,
                cx,
            );
            let oauth_client_metadata_url = new_input(
                "https://client.example/mcpstore.json",
                &draft.oauth_client_metadata_url,
                window,
                cx,
            );
            let agent_id = new_input(tr(cx, crate::dict::TextKey::Store), "", window, cx);
            let scope_label_text = match &mode {
                FormMode::Add => String::new(),
                FormMode::Edit { scope, .. } => match scope {
                    ScopeRef::Store => tr(cx, crate::dict::TextKey::Store).to_string(),
                    ScopeRef::Agent { agent_id } => {
                        format!("{} {agent_id}", tr(cx, crate::dict::TextKey::Agent))
                    }
                },
            };
            let scope_label = new_input("", &scope_label_text, window, cx);
            let transport_text = new_input("", if transport_http { "http" } else { "stdio" }, window, cx);
            let handshake = new_select(&HANDSHAKE_UI, &draft.handshake_mode, window, cx);
            let startup = new_select(&STARTUP_POLICIES, &draft.startup_policy, window, cx);
            let restart = new_select(&RESTART_POLICIES, &draft.restart_policy, window, cx);
            let preview = new_area("", &preview_text, window, cx);

            let mut subs = Vec::new();
            watch_text(&mut subs, &name, window, cx);
            watch_text(&mut subs, &command, window, cx);
            watch_text(&mut subs, &url, window, cx);
            watch_text(&mut subs, &description, window, cx);
            watch_text(&mut subs, &working_dir, window, cx);
            watch_text(&mut subs, &oauth_client_id, window, cx);
            watch_text(&mut subs, &oauth_client_metadata_url, window, cx);
            watch_text(&mut subs, &agent_id, window, cx);
            watch_select(&mut subs, &handshake, window, cx);
            watch_select(&mut subs, &startup, window, cx);
            watch_select(&mut subs, &restart, window, cx);
            subs.push(cx.subscribe_in(&preview, window, |this, state, event, window, cx| {
                if this.writing_preview || !matches!(event, InputEvent::Change) {
                    return;
                }
                let text = state.read(cx).value().to_string();
                this.apply_preview_text(text, window, cx);
            }));

            let mut next_row = 1u64;
            let args_rows = lines_from_text(&draft.args_text)
                .into_iter()
                .map(|line| {
                    let id = next_row;
                    next_row += 1;
                    let input = new_input("-y", &line, window, cx);
                    let sub = bind_change(&input, window, cx);
                    LineRow { id, input, _sub: sub }
                })
                .collect();
            let env_rows = pairs_from_text(&draft.env_text)
                .into_iter()
                .map(|(key, value)| {
                    let id = next_row;
                    next_row += 1;
                    let key = new_input("TOKEN", &key, window, cx);
                    let value = new_input("...", &value, window, cx);
                    let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                    PairRow { id, key, value, _subs: subs }
                })
                .collect();
            let header_rows = pairs_from_text(&draft.headers_text)
                .into_iter()
                .map(|(key, value)| {
                    let id = next_row;
                    next_row += 1;
                    let key = new_input("Authorization", &key, window, cx);
                    let value = new_area("Bearer ...", &value, window, cx);
                    let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                    HeaderRow { id, key, value, _subs: subs }
                })
                .collect();

            Self {
                api: api.clone(),
                mode,
                name,
                transport_http,
                command,
                args_rows,
                url,
                description,
                working_dir,
                env_rows,
                header_rows,
                next_row,
                oauth_enabled: draft.oauth_enabled,
                oauth_client_id,
                oauth_client_metadata_url,
                handshake,
                startup,
                restart,
                scope_agent: false,
                agent_id,
                scope_label,
                transport_text,
                agents: Vec::new(),
                open_env: false,
                open_cwd: false,
                open_headers: false,
                open_oauth: draft.oauth_enabled,
                open_desc: false,
                open_handshake: false,
                open_scope: false,
                open_lifecycle: false,
                preview_format: ServiceConfigFormat::Json,
                preview,
                preview_error: None,
                preview_dirty: false,
                writing_preview: false,
                error: None,
                done: false,
                running: false,
                _subs: subs,
            }
        });

        // Prefetch agent ids for scope picker (add mode).
        if matches!(entity.read(cx).mode, FormMode::Add) {
            let api2 = api.clone();
            let weak = entity.downgrade();
            cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
                let result = cx
                    .background_spawn(async move { api2.list_agents().await })
                    .await;
                weak.update(cx, |this, cx| {
                    if let Ok(agents) = result {
                        this.agents = agents.into_iter().map(|a| a.agent_id).collect();
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }

        let entity2 = entity.clone();
        let viewport = window.viewport_size();
        // Width follows the two columns, not the React 68rem cap.
        let dialog_w = px(400. + ASIDE_W + 60.).min(viewport.width - px(48.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(dialog_w)
                .pl(px(0.))
                .pr(px(0.))
                .pt(px(0.))
                .pb(px(0.))
                .child(entity2.clone())
        });
    }

    fn draft(&self, cx: &gpui_kit::App) -> ServiceFormDraft {
        let text = |state: &Entity<InputState>| state.read(cx).value().trim().to_string();
        ServiceFormDraft {
            transport: if self.transport_http {
                "streamable-http".into()
            } else {
                "stdio".into()
            },
            command: text(&self.command),
            args_text: self.args_text(cx),
            url: text(&self.url),
            description: text(&self.description),
            working_dir: text(&self.working_dir),
            env_text: self.env_text(cx),
            headers_text: self.headers_text(cx),
            oauth_enabled: self.oauth_enabled,
            oauth_client_id: text(&self.oauth_client_id),
            oauth_client_metadata_url: text(&self.oauth_client_metadata_url),
            handshake_mode: select_value(&self.handshake, HANDSHAKE_MODES[0], cx),
            startup_policy: select_value(&self.startup, STARTUP_POLICIES[0], cx),
            restart_policy: select_value(&self.restart, RESTART_POLICIES[0], cx),
        }
    }

    fn agent_id_opt(&self, cx: &gpui_kit::App) -> Option<String> {
        if !self.scope_agent {
            return None;
        }
        let id = self.agent_id.read(cx).value().trim().to_string();
        if id.is_empty() { None } else { Some(id) }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_row;
        self.next_row += 1;
        id
    }

    fn args_text(&self, cx: &gpui_kit::App) -> String {
        lines_to_text(
            &self
                .args_rows
                .iter()
                .map(|row| row.input.read(cx).value().to_string())
                .collect::<Vec<_>>(),
        )
    }

    fn env_text(&self, cx: &gpui_kit::App) -> String {
        pairs_to_text(
            &self
                .env_rows
                .iter()
                .map(|row| (row.key.read(cx).value().to_string(), row.value.read(cx).value().to_string()))
                .collect::<Vec<_>>(),
        )
    }

    fn headers_text(&self, cx: &gpui_kit::App) -> String {
        pairs_to_text(
            &self
                .header_rows
                .iter()
                .map(|row| (row.key.read(cx).value().to_string(), row.value.read(cx).value().to_string()))
                .collect::<Vec<_>>(),
        )
    }

    fn replace_lines(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.args_text(cx) == text {
            return;
        }
        self.args_rows.clear();
        for line in lines_from_text(text) {
            let id = self.alloc_id();
            let input = new_input("-y", &line, window, cx);
            let sub = bind_change(&input, window, cx);
            self.args_rows.push(LineRow { id, input, _sub: sub });
        }
    }

    fn replace_pairs(&mut self, text: &str, headers: bool, window: &mut Window, cx: &mut Context<Self>) {
        if headers {
            if self.headers_text(cx) == text {
                return;
            }
            self.header_rows.clear();
            for (key, value) in pairs_from_text(text) {
                let id = self.alloc_id();
                let key = new_input("Authorization", &key, window, cx);
                let value = new_area("Bearer ...", &value, window, cx);
                let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                self.header_rows.push(HeaderRow { id, key, value, _subs: subs });
            }
        } else {
            if self.env_text(cx) == text {
                return;
            }
            self.env_rows.clear();
            for (key, value) in pairs_from_text(text) {
                let id = self.alloc_id();
                let key = new_input("TOKEN", &key, window, cx);
                let value = new_input("...", &value, window, cx);
                let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                self.env_rows.push(PairRow { id, key, value, _subs: subs });
            }
        }
    }

    fn insert_line_after(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.args_rows.iter().position(|row| row.id == id).map(|i| i + 1).unwrap_or(self.args_rows.len());
        let id = self.alloc_id();
        let input = new_input("-y", "", window, cx);
        let sub = bind_change(&input, window, cx);
        self.args_rows.insert(at, LineRow { id, input, _sub: sub });
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn remove_line(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.args_rows.retain(|row| row.id != id);
        if self.args_rows.is_empty() {
            let id = self.alloc_id();
            let input = new_input("-y", "", window, cx);
            let sub = bind_change(&input, window, cx);
            self.args_rows.push(LineRow { id, input, _sub: sub });
        }
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn insert_pair_after(&mut self, id: u64, headers: bool, window: &mut Window, cx: &mut Context<Self>) {
        if headers {
            let at = self.header_rows.iter().position(|row| row.id == id).map(|i| i + 1).unwrap_or(self.header_rows.len());
            let id = self.alloc_id();
            let key = new_input("Authorization", "", window, cx);
            let value = new_area("Bearer ...", "", window, cx);
            let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
            self.header_rows.insert(at, HeaderRow { id, key, value, _subs: subs });
        } else {
            let at = self.env_rows.iter().position(|row| row.id == id).map(|i| i + 1).unwrap_or(self.env_rows.len());
            let id = self.alloc_id();
            let key = new_input("TOKEN", "", window, cx);
            let value = new_input("...", "", window, cx);
            let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
            self.env_rows.insert(at, PairRow { id, key, value, _subs: subs });
        }
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn remove_pair(&mut self, id: u64, headers: bool, window: &mut Window, cx: &mut Context<Self>) {
        if headers {
            self.header_rows.retain(|row| row.id != id);
            if self.header_rows.is_empty() {
                let id = self.alloc_id();
                let key = new_input("Authorization", "", window, cx);
                let value = new_area("Bearer ...", "", window, cx);
                let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                self.header_rows.push(HeaderRow { id, key, value, _subs: subs });
            }
        } else {
            self.env_rows.retain(|row| row.id != id);
            if self.env_rows.is_empty() {
                let id = self.alloc_id();
                let key = new_input("TOKEN", "", window, cx);
                let value = new_input("...", "", window, cx);
                let subs = vec![bind_change(&key, window, cx), bind_change(&value, window, cx)];
                self.env_rows.push(PairRow { id, key, value, _subs: subs });
            }
        }
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn sync_preview_from_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.preview_dirty {
            cx.notify();
            return;
        }
        let draft = self.draft(cx);
        let text = serialize_service_config(&draft, self.preview_format);
        self.writing_preview = true;
        self.preview
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.writing_preview = false;
        self.preview_error = None;
        cx.notify();
    }

    fn apply_preview_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_dirty = true;
        match parse_imported_service_config_format(self.preview_format, &text) {
            Ok((name, config)) => {
                let draft = ServiceFormDraft::from_config(&config);
                self.apply_draft(draft, name, window, cx);
                self.preview_dirty = false;
                self.preview_error = None;
            }
            Err(message) => {
                self.preview_error = Some(message.into());
            }
        }
        cx.notify();
    }

    fn apply_draft(
        &mut self,
        draft: ServiceFormDraft,
        imported_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.name.read(cx).value().trim().is_empty() {
            if let Some(name) = imported_name {
                set_input(&self.name, name, window, cx);
            }
        }
        self.transport_http = draft.transport != "stdio";
        set_input(
            &self.transport_text,
            if self.transport_http { "http" } else { "stdio" }.into(),
            window,
            cx,
        );
        self.oauth_enabled = draft.oauth_enabled;
        set_input(&self.command, draft.command, window, cx);
        set_input(&self.url, draft.url, window, cx);
        set_input(&self.description, draft.description, window, cx);
        set_input(&self.working_dir, draft.working_dir, window, cx);
        set_input(&self.oauth_client_id, draft.oauth_client_id, window, cx);
        set_input(
            &self.oauth_client_metadata_url,
            draft.oauth_client_metadata_url,
            window,
            cx,
        );
        self.replace_lines(&draft.args_text, window, cx);
        self.replace_pairs(&draft.env_text, false, window, cx);
        self.replace_pairs(&draft.headers_text, true, window, cx);
        self.handshake.update(cx, |state, cx| {
            state.set_selected_index(
                Some(option_index(&HANDSHAKE_UI, &draft.handshake_mode)),
                window,
                cx,
            );
        });
        self.startup.update(cx, |state, cx| {
            state.set_selected_index(
                Some(option_index(&STARTUP_POLICIES, &draft.startup_policy)),
                window,
                cx,
            );
        });
        self.restart.update(cx, |state, cx| {
            state.set_selected_index(
                Some(option_index(&RESTART_POLICIES, &draft.restart_policy)),
                window,
                cx,
            );
        });
    }

    fn set_preview_format(&mut self, format: ServiceConfigFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_format = format;
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn set_transport_http(&mut self, http: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.transport_http = http;
        if http {
            self.replace_pairs("", false, window, cx);
        }
        set_input(
            &self.transport_text,
            if http { "http" } else { "stdio" }.into(),
            window,
            cx,
        );
        self.preview_dirty = false;
        self.sync_preview_from_form(window, cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        let draft = self.draft(cx);
        let agent_id = self.agent_id_opt(cx);
        if name.is_empty() {
            self.error = Some("name is required".into());
            cx.notify();
            return;
        }
        if self.scope_agent && agent_id.is_none() && matches!(self.mode, FormMode::Add) {
            self.error = Some("Agent ID is required".into());
            cx.notify();
            return;
        }
        if let Err(message) = draft.validate() {
            self.error = Some(message.into());
            cx.notify();
            return;
        }
        self.running = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        let mode = self.mode.clone();
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    match mode {
                        FormMode::Add => {
                            api.add_service(&name, &draft, agent_id.as_deref()).await
                        }
                        FormMode::Edit { name, scope, .. } => {
                            api.update_service_scope(&name, &scope, &draft).await
                        }
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(_) => this.done = true,
                    Err(e) => this.error = Some(format!("{e:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn field_label(color: gpui_kit::Hsla, label: &'static str) -> gpui_kit::Div {
    div()
        .text_sm()
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(color)
        .child(label)
}

fn labeled_input(
    color: gpui_kit::Hsla,
    label: &'static str,
    input: &Entity<InputState>,
) -> impl IntoElement {
    v_flex()
        .gap_1p5()
        .child(field_label(color, label))
        .child(Input::new(input))
}

fn input_addon(label: &'static str, color: gpui_kit::Hsla) -> gpui_kit::Div {
    div()
        .pl_2()
        .text_xs()
        .font_family("Monaco")
        .text_color(color)
        .child(label)
}

fn row_buttons(
    id: u64,
    prefix: &'static str,
    on_add: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    on_remove: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> gpui_kit::Div {
    h_flex()
        .flex_shrink_0()
        .child(
            Button::new(SharedString::from(format!("{prefix}-add-{id}")))
                .ghost()
                .with_size(Size::XSmall)
                .icon(IconName::Plus)
                .on_click(on_add),
        )
        .child(
            Button::new(SharedString::from(format!("{prefix}-del-{id}")))
                .ghost()
                .with_size(Size::XSmall)
                .icon(IconName::Trash)
                .on_click(on_remove),
        )
}

fn kv_header(color: gpui_kit::Hsla, key: &'static str, value: &'static str) -> gpui_kit::Div {
    h_flex()
        .gap_2()
        .items_center()
        .text_xs()
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(color)
        .child(div().w(relative(0.38)).flex_shrink_0().child(key))
        .child(div().flex_1().min_w_0().child(value))
        .child(div().w(px(52.)).flex_shrink_0())
}

fn collapsible_block(
    id: &'static str,
    title: &'static str,
    open: bool,
    muted: gpui_kit::Hsla,
    fg: gpui_kit::Hsla,
    on_toggle: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    Collapsible::new()
        .open(open)
        .child(
            h_flex()
                .id(id)
                .gap_1p5()
                .items_center()
                .cursor_pointer()
                .on_click(on_toggle)
                .child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .text_color(muted)
                    .with_size(Size::XSmall),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(fg)
                        .child(title),
                ),
        )
        .content(div().pt_2().child(content))
}

impl Render for ServiceFormDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (muted, fg) = (cx.theme().muted_foreground, cx.theme().foreground);
        let border = cx.theme().border;
        let viewport = window.viewport_size();
        let form_h = viewport.height * 0.78;
        let is_stdio = !self.transport_http;
        let is_add = matches!(self.mode, FormMode::Add);
        let draft = self.draft(cx);
        let agent_id = self.agent_id_opt(cx);
        let service_name = {
            let n = self.name.read(cx).value().trim().to_string();
            if n.is_empty() { "github".into() } else { n }
        };
        let cli = build_add_service_cli(&service_name, &draft, agent_id.as_deref());
        let preview_hint = self.preview_error.clone().unwrap_or_else(|| {
            SharedString::from(match self.preview_format {
                ServiceConfigFormat::Json => tr(cx, crate::dict::TextKey::FieldHelpJson),
                ServiceConfigFormat::Toml => tr(cx, crate::dict::TextKey::FieldHelpToml),
            })
        });
        let preview_has_error = self.preview_error.is_some();
        let title = tr(
            cx,
            if is_add {
                crate::dict::TextKey::NavAddService
            } else {
                crate::dict::TextKey::EditService
            },
        );
        let submit_label = if self.running {
            tr(
                cx,
                if is_add {
                    crate::dict::TextKey::Adding
                } else {
                    crate::dict::TextKey::Saving
                },
            )
        } else if is_add {
            tr(cx, crate::dict::TextKey::NavAddService)
        } else {
            tr(cx, crate::dict::TextKey::Save)
        };

        let transport_tabs = TabBar::new("service-form-transport")
            .segmented()
            .with_size(Size::Small)
            .w(px(160.))
            .selected_index(if self.transport_http { 1 } else { 0 })
            .children(["stdio", "http"])
            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                this.set_transport_http(*ix == 1, window, cx);
            }));

        // Left column is the React scroll pane (25rem) + FieldGroup gap-5.
        let mut left = v_flex()
            .id("service-form-fields")
            .gap_5()
            .flex_1()
            .max_w(px(400.))
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .pr_1();

        if is_add {
            left = left.child(
                h_flex()
                    .gap_4()
                    .items_end()
                    .child(
                        v_flex()
                            .gap_1p5()
                            .flex_1()
                            .min_w_0()
                            .child(field_label(fg, tr(cx, crate::dict::TextKey::Name)))
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(field_label(fg, tr(cx, crate::dict::TextKey::Transport)))
                            .child(transport_tabs),
                    ),
            );
        } else {
            left = left
                .child(
                    h_flex().gap_4().child(
                        v_flex()
                            .gap_1p5()
                            .flex_1()
                            .min_w_0()
                            .child(field_label(fg, tr(cx, crate::dict::TextKey::Name)))
                            .child(Input::new(&self.name).readonly(true)),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .flex_1()
                            .min_w_0()
                            .child(field_label(fg, tr(cx, crate::dict::TextKey::Scope)))
                            .child(Input::new(&self.scope_label).readonly(true)),
                    ),
                )
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(field_label(fg, tr(cx, crate::dict::TextKey::Transport)))
                        .child(Input::new(&self.transport_text).readonly(true)),
                );
        }

        // ServiceConfigFormFields: gap-4, scope extra last.
        let mut config = v_flex().gap_4();
        if is_stdio {
            let mut args_rows = v_flex().gap_2();
            for row in &self.args_rows {
                let id = row.id;
                args_rows = args_rows.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().min_w_0().child(Input::new(&row.input)))
                        .child(row_buttons(
                            id,
                            "arg",
                            cx.listener(move |this, _, window, cx| {
                                this.insert_line_after(id, window, cx);
                            }),
                            cx.listener(move |this, _, window, cx| {
                                this.remove_line(id, window, cx);
                            }),
                        )),
                );
            }
            let mut env_rows = v_flex()
                .gap_2()
                .child(kv_header(
                    muted,
                    tr(cx, crate::dict::TextKey::KvKey),
                    tr(cx, crate::dict::TextKey::KvValue),
                ));
            for row in &self.env_rows {
                let id = row.id;
                env_rows = env_rows.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().w(relative(0.38)).flex_shrink_0().child(Input::new(&row.key)))
                        .child(div().flex_1().min_w_0().child(Input::new(&row.value)))
                        .child(row_buttons(
                            id,
                            "env",
                            cx.listener(move |this, _, window, cx| {
                                this.insert_pair_after(id, false, window, cx);
                            }),
                            cx.listener(move |this, _, window, cx| {
                                this.remove_pair(id, false, window, cx);
                            }),
                        )),
                );
            }
            config = config
                .child(labeled_input(fg, tr(cx, crate::dict::TextKey::Command), &self.command))
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(field_label(fg, tr(cx, crate::dict::TextKey::Args)))
                        .child(v_flex().gap_3().child(args_rows)),
                )
                .child(collapsible_block(
                    "collapse-env",
                    tr(cx, crate::dict::TextKey::EnvVars),
                    self.open_env,
                    muted,
                    fg,
                    cx.listener(|this, _, _, cx| {
                        this.open_env = !this.open_env;
                        cx.notify();
                    }),
                    v_flex().gap_3().child(env_rows),
                ))
                .child(collapsible_block(
                    "collapse-cwd",
                    tr(cx, crate::dict::TextKey::WorkingDirectory),
                    self.open_cwd,
                    muted,
                    fg,
                    cx.listener(|this, _, _, cx| {
                        this.open_cwd = !this.open_cwd;
                        cx.notify();
                    }),
                    Input::new(&self.working_dir).prefix(input_addon("cwd", muted)),
                ));
        } else {
            let mut header_rows = v_flex()
                .gap_2()
                .child(kv_header(
                    muted,
                    tr(cx, crate::dict::TextKey::KvKey),
                    tr(cx, crate::dict::TextKey::KvValue),
                ));
            for row in &self.header_rows {
                let id = row.id;
                header_rows = header_rows.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(div().w(relative(0.38)).flex_shrink_0().child(Input::new(&row.key)))
                        .child(
                            div().flex_1().min_w_0().child(
                                Textarea::new(&row.value).h(px(64.)),
                            ),
                        )
                        .child(row_buttons(
                            id,
                            "hdr",
                            cx.listener(move |this, _, window, cx| {
                                this.insert_pair_after(id, true, window, cx);
                            }),
                            cx.listener(move |this, _, window, cx| {
                                this.remove_pair(id, true, window, cx);
                            }),
                        )),
                );
            }
            config = config
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(field_label(fg, tr(cx, crate::dict::TextKey::HttpEndpoint)))
                        .child(Input::new(&self.url).prefix(input_addon("url", muted))),
                )
                .child(collapsible_block(
                    "collapse-headers",
                    tr(cx, crate::dict::TextKey::Headers),
                    self.open_headers,
                    muted,
                    fg,
                    cx.listener(|this, _, _, cx| {
                        this.open_headers = !this.open_headers;
                        cx.notify();
                    }),
                    v_flex().gap_3().child(header_rows),
                ))
                .child(collapsible_block(
                    "collapse-oauth",
                    tr(cx, crate::dict::TextKey::OauthEnabled),
                    self.open_oauth,
                    muted,
                    fg,
                    cx.listener(|this, _, _, cx| {
                        this.open_oauth = !this.open_oauth;
                        cx.notify();
                    }),
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted)
                                        .child(tr(cx, crate::dict::TextKey::OauthEnabledHint)),
                                )
                                .child(
                                    Switch::new("service-form-oauth")
                                        .checked(self.oauth_enabled)
                                        .on_click(cx.listener(|this, checked, window, cx| {
                                            this.oauth_enabled = *checked;
                                            this.preview_dirty = false;
                                            this.sync_preview_from_form(window, cx);
                                        })),
                                ),
                        )
                        .when(self.oauth_enabled, |el| {
                            el.child(labeled_input(
                                fg,
                                tr(cx, crate::dict::TextKey::OauthClientId),
                                &self.oauth_client_id,
                            ))
                            .child(labeled_input(
                                fg,
                                tr(cx, crate::dict::TextKey::OauthClientMetadataUrl),
                                &self.oauth_client_metadata_url,
                            ))
                            .child(
                                div().text_xs().text_color(muted).child(tr(
                                    cx,
                                    crate::dict::TextKey::OauthClientMetadataUrlHint,
                                )),
                            )
                        }),
                ));
        }

        config = config
            .child(collapsible_block(
                "collapse-desc",
                tr(cx, crate::dict::TextKey::Description),
                self.open_desc,
                muted,
                fg,
                cx.listener(|this, _, _, cx| {
                    this.open_desc = !this.open_desc;
                    cx.notify();
                }),
                Input::new(&self.description),
            ))
            .child(collapsible_block(
                "collapse-handshake",
                tr(cx, crate::dict::TextKey::HandshakeMode),
                self.open_handshake,
                muted,
                fg,
                cx.listener(|this, _, _, cx| {
                    this.open_handshake = !this.open_handshake;
                    cx.notify();
                }),
                v_flex()
                    .gap_1p5()
                    .child(Select::new(&self.handshake).id("service-form-handshake").w(px(176.)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(tr(cx, crate::dict::TextKey::HandshakeModeHint)),
                    ),
            ));

        if is_add {
            let agent_input = Input::new(&self.agent_id).disabled(!self.scope_agent);
            let agent_input = if self.agents.is_empty() {
                agent_input
            } else {
                let entity = cx.entity();
                let agents = self.agents.clone();
                let enabled = self.scope_agent;
                agent_input.suffix(
                    Button::new("service-form-agent-pick")
                        .ghost()
                        .with_size(Size::XSmall)
                        .icon(IconName::ChevronDown)
                        .disabled(!enabled)
                        .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                            agents.iter().cloned().fold(menu, |menu, id| {
                                let entity = entity.clone();
                                menu.item(PopupMenuItem::new(id.clone()).on_click(move |_, window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.agent_id.update(cx, |state, cx| {
                                            state.set_value(id.clone(), window, cx);
                                        });
                                        this.preview_dirty = false;
                                        this.sync_preview_from_form(window, cx);
                                    });
                                }))
                            })
                        }),
                )
            };
            config = config.child(collapsible_block(
                "collapse-scope",
                tr(cx, crate::dict::TextKey::Scope),
                self.open_scope,
                muted,
                fg,
                cx.listener(|this, _, _, cx| {
                    this.open_scope = !this.open_scope;
                    cx.notify();
                }),
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(muted)
                            .child(tr(cx, crate::dict::TextKey::Agent)),
                    )
                    .child(
                        Switch::new("service-form-scope-agent")
                            .checked(self.scope_agent)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.scope_agent = *checked;
                                let agent = this.scope_agent;
                                let placeholder = if agent {
                                    tr(cx, crate::dict::TextKey::AgentIdPlaceholder)
                                } else {
                                    tr(cx, crate::dict::TextKey::Store)
                                };
                                this.agent_id.update(cx, |state, cx| {
                                    if !agent {
                                        state.set_value(String::new(), window, cx);
                                    }
                                    state.set_placeholder(placeholder, window, cx);
                                });
                                this.preview_dirty = false;
                                this.sync_preview_from_form(window, cx);
                            })),
                    )
                    .child(div().flex_1().min_w_0().child(agent_input)),
            ));
        }

        left = left.child(config);
        if is_add {
            left = left.child(collapsible_block(
                "collapse-lifecycle",
                tr(cx, crate::dict::TextKey::LifecyclePolicy),
                self.open_lifecycle,
                muted,
                fg,
                cx.listener(|this, _, _, cx| {
                    this.open_lifecycle = !this.open_lifecycle;
                    cx.notify();
                }),
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1p5()
                            .flex_1()
                            .child(field_label(fg, "startup_policy"))
                            .child(Select::new(&self.startup).id("service-form-startup").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .flex_1()
                            .child(field_label(fg, "restart_policy"))
                            .child(Select::new(&self.restart).id("service-form-restart").w_full()),
                    ),
            ));
        }

        let copy_label = tr(cx, crate::dict::TextKey::Copy);
        let cli_card = v_flex()
            .gap_0()
            .flex_shrink_0()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x27272a))
            .bg(rgb(0x09090b))
            .overflow_hidden()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0x27272a))
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgb(0x0ea5e9).opacity(0.15))
                            .text_xs()
                            .font_family("Monaco")
                            .text_color(rgb(0x7dd3fc))
                            .child("ADD"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_family("Monaco")
                            .text_color(rgb(0xd4d4d8))
                            .child(service_name.clone()),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0x3f3f46))
                            .text_xs()
                            .font_family("Monaco")
                            .text_color(rgb(0xa1a1aa))
                            .child(tr(cx, crate::dict::TextKey::CliRequestLabel)),
                    )
                    .child(
                        Button::new("service-form-cli-copy")
                            .ghost()
                            .with_size(Size::Small)
                            .icon(IconName::Copy)
                            .label(copy_label)
                            .text_color(rgb(0xd4d4d8))
                            .on_click({
                                let cli = cli.clone();
                                move |_, _, cx| {
                                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(cli.clone()));
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .id("service-form-cli-body")
                    .p_4()
                    .max_h(px(144.))
                    .overflow_y_scroll()
                    .font_family("Monaco")
                    .text_xs()
                    .text_color(rgb(0xf4f4f5))
                    .child(cli),
            );

        let format_tabs = TabBar::new("service-form-preview-format")
            .segmented()
            .with_size(Size::XSmall)
            .selected_index(match self.preview_format {
                ServiceConfigFormat::Json => 0,
                ServiceConfigFormat::Toml => 1,
            })
            .children(["json", "toml"])
            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                this.set_preview_format(
                    if *ix == 0 {
                        ServiceConfigFormat::Json
                    } else {
                        ServiceConfigFormat::Toml
                    },
                    window,
                    cx,
                );
            }));

        let preview_text = self.preview.read(cx).value().to_string();
        let preview_card = v_flex()
            .gap_0()
            .flex_1()
            .min_h_0()
            .rounded_lg()
            .border_1()
            .border_color(border)
            .bg(cx.theme().muted.opacity(0.3))
            .overflow_hidden()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .items_center()
                    .gap_2()
                    .bg(cx.theme().muted.opacity(0.5))
                    .child(format_tabs)
                    .child(div().flex_1())
                    .child(
                        Button::new("service-form-preview-copy")
                            .ghost()
                            .with_size(Size::Small)
                            .icon(IconName::Copy)
                            .label(copy_label)
                            .on_click({
                                let preview_text = preview_text.clone();
                                move |_, _, cx| {
                                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                        preview_text.clone(),
                                    ));
                                }
                            }),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    Textarea::new(&self.preview)
                        .appearance(false)
                        .h(relative(1.))
                        .p_4()
                        .font_family("Monaco")
                        .text_sm(),
                ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(if preview_has_error { cx.theme().danger } else { muted })
                    .child(preview_hint),
            );

        let mut right = v_flex().gap_4().w(px(ASIDE_W)).flex_shrink_0().min_h_0();
        if is_add {
            right = right.child(cli_card);
        }
        right = right.child(preview_card);

        v_flex()
            .w_full()
            .h(form_h)
            .overflow_hidden()
            .text_color(fg)
            .child(
                div()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(border)
                    .px_5()
                    .py_4()
                    .pr(px(40.))
                    .text_lg()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                h_flex()
                    .id("service-form-body")
                    .flex_1()
                    .min_h_0()
                    .gap_5()
                    .px_5()
                    .py_4()
                    .items_stretch()
                    .child(left)
                    .child(right),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .px_5()
                        .pb_2()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .when(self.done, |el| {
                el.child(
                    div()
                        .px_5()
                        .pb_2()
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child(tr(cx, crate::dict::TextKey::ListRefreshNotice)),
                )
            })
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .justify_end()
                    .border_t_1()
                    .border_color(border)
                    .px_5()
                    .py(px(14.))
                    .child(
                        Button::new("service-form-cancel")
                            .outline()
                            .label(tr(cx, crate::dict::TextKey::Cancel))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("service-form-submit")
                            .primary()
                            .label(submit_label)
                            .disabled(self.running || self.done)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_codecs_match_react() {
        assert_eq!(lines_from_text(""), vec![""]);
        assert_eq!(lines_from_text("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(lines_to_text(&[String::new()]), "");
        assert_eq!(lines_to_text(&["-y".into(), String::new()]), "-y\n");

        assert_eq!(pairs_from_text(""), vec![("".into(), "".into())]);
        assert_eq!(pairs_from_text("\n  \n"), vec![("".into(), "".into())]);
        assert_eq!(
            pairs_from_text("TOKEN=ab\n=bad\nplain\n  K = v "),
            vec![
                ("TOKEN".into(), "ab".into()),
                ("=bad".into(), "".into()),
                ("plain".into(), "".into()),
                ("K".into(), " v".into()),
            ]
        );
        assert_eq!(
            pairs_to_text(&[
                ("".into(), "".into()),
                ("A".into(), "b".into()),
                ("".into(), " x".into()),
            ]),
            "A=b\n= x"
        );
    }
}
