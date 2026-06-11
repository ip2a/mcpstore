use mcpstore::registry::ConnectionStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{FocusArea, LogsPane, LogsSection, TuiApp},
    i18n::{self, TextKey},
    layout as tui_layout, theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let line = Line::from(vec![
        Span::styled(
            if app.focus_area == FocusArea::ViewFilter {
                "> "
            } else {
                "  "
            },
            theme::accent(),
        ),
        Span::styled(
            i18n::text(app.locale, TextKey::FocusControlBar),
            theme::muted(),
        ),
        Span::raw("  "),
        Span::styled("r 刷新  Enter 进入  Esc 返回", theme::text()),
    ]);
    widgets::chrome::render_control_bar(frame, area, app, line);
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tui_layout::CONTENT_MENU_PERCENT),
            Constraint::Percentage(tui_layout::CONTENT_BODY_PERCENT),
        ])
        .split(area);

    render_menu(frame, layout[0], app);
    render_body(frame, layout[1], app);
}

fn render_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.logs_pane == LogsPane::Menu;
    let items = LogsSection::ALL.iter().map(|section| {
        let selected = *section == app.logs_section;
        ListItem::new(Line::from(vec![
            Span::styled(if selected { "> " } else { "  " }, theme::accent()),
            Span::styled(
                section.label(app.locale),
                if selected {
                    theme::menu_selected()
                } else {
                    theme::text()
                },
            ),
        ]))
        .style(if selected {
            theme::menu_selected()
        } else {
            theme::text()
        })
    });

    let menu = List::new(items)
        .block(
            widgets::chrome::panel_block(i18n::text(app.locale, TextKey::NavLogs), focused)
                .padding(Padding::horizontal(1)),
        )
        .style(theme::text());
    frame.render_widget(menu, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.logs_pane == LogsPane::Body;
    let title = format!(
        "{} / {}",
        i18n::text(app.locale, TextKey::ContentRegion),
        app.logs_section.label(app.locale)
    );

    match app.logs_section {
        LogsSection::Runtime => render_list(
            frame,
            area,
            title,
            focused,
            runtime_items(app),
            i18n::text(app.locale, TextKey::LogsEmpty),
        ),
        LogsSection::StoreEvents => render_list(
            frame,
            area,
            title,
            focused,
            app.store_event_history
                .iter()
                .map(|line| ListItem::new(Line::from(line.clone())))
                .collect(),
            i18n::text(app.locale, TextKey::LogsStoreEventsEmpty),
        ),
        LogsSection::Services => render_list(
            frame,
            area,
            title,
            focused,
            service_items(app),
            i18n::text(app.locale, TextKey::LogsEmpty),
        ),
        LogsSection::Config => render_config(frame, area, app, title, focused),
    }
}

fn render_list(
    frame: &mut Frame,
    area: Rect,
    title: String,
    focused: bool,
    mut items: Vec<ListItem>,
    empty: &'static str,
) {
    if items.is_empty() {
        items.push(ListItem::new(Line::from(empty)));
    }

    let list = List::new(items)
        .block(widgets::chrome::panel_block(title, focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(list, area);
}

fn render_config(frame: &mut Frame, area: Rect, app: &TuiApp, title: String, focused: bool) {
    let lines = log_config_lines(app);
    let paragraph = Paragraph::new(lines)
        .block(widgets::chrome::panel_block(title, focused).padding(Padding::horizontal(1)))
        .wrap(Wrap { trim: true })
        .style(theme::text());
    frame.render_widget(paragraph, area);
}

fn runtime_items(app: &TuiApp) -> Vec<ListItem<'static>> {
    app.status_history
        .iter()
        .rev()
        .map(|message| ListItem::new(Line::from(message.clone())))
        .collect()
}

fn service_items(app: &TuiApp) -> Vec<ListItem<'static>> {
    let mut items = Vec::new();
    let total = app.all_services.len();
    let connected = app
        .all_services
        .iter()
        .filter(|service| service.status == ConnectionStatus::Connected)
        .count();
    let errors = app
        .all_services
        .iter()
        .filter(|service| service.status == ConnectionStatus::Error)
        .count();
    items.push(ListItem::new(Line::from(format!(
        "summary  total={total} connected={connected} error={errors}"
    ))));

    for service in &app.all_services {
        items.push(ListItem::new(Line::from(format!(
            "{}  transport={}  status={:?}  endpoint={}",
            service.name, service.transport, service.status, service.endpoint
        ))));
    }

    if let Some(detail) = app.selected_detail.as_ref() {
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(format!(
            "selected  {}  health={}  attempts={}  latency={}",
            detail.title, detail.health_status, detail.attempts, detail.latency
        ))));
        if detail.error_message != "-" {
            items.push(ListItem::new(Line::from(format!(
                "selected_error  {}",
                detail.error_message
            ))));
        }
    }

    items
}

pub fn log_config_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let manager = app.store.config_manager();
    let config = manager.load_app_config_or_default().ok();
    let server_log_level = config
        .as_ref()
        .map(|config| config.server.log_level.clone())
        .unwrap_or_else(|| "-".to_string());
    let standalone_log_level = config
        .as_ref()
        .map(|config| config.standalone.log_level.clone())
        .unwrap_or_else(|| "-".to_string());
    let standalone_log_format = config
        .as_ref()
        .map(|config| config.standalone.log_format.clone())
        .unwrap_or_else(|| "-".to_string());
    let debug_enabled = config
        .as_ref()
        .map(|config| config.standalone.enable_debug.to_string())
        .unwrap_or_else(|| "-".to_string());

    vec![
        kv_line(app, TextKey::SettingsServerLogLevel, server_log_level),
        kv_line(
            app,
            TextKey::SettingsStandaloneLogLevel,
            standalone_log_level,
        ),
        kv_line(
            app,
            TextKey::SettingsStandaloneLogFormat,
            standalone_log_format,
        ),
        kv_line(app, TextKey::SettingsDebugEnabled, debug_enabled),
        kv_line(app, TextKey::SettingsTracingSink, "stderr".to_string()),
        kv_line(app, TextKey::SettingsLogFile, "not configured".to_string()),
        kv_line(
            app,
            TextKey::SettingsConfigFile,
            manager.app_config_path().display().to_string(),
        ),
    ]
}

fn kv_line(app: &TuiApp, key: TextKey, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{}: ", i18n::text(app.locale, key)), theme::muted()),
        Span::styled(value, theme::text()),
    ])
}
