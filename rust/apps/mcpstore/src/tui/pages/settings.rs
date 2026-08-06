use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{FocusArea, SettingsPane, SettingsSection, TuiApp},
    i18n::{self, Locale, TextKey},
    pages::logs,
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    widgets::chrome::render_control_bar(
        frame,
        area,
        app,
        Line::from(vec![
            focus_prefix(app.focus_area == FocusArea::ViewFilter),
            Span::styled("Settings  ", theme::field_label()),
            Span::styled(
                "h/l Focus  j/k Section  Enter Edit  Esc Back  q Quit",
                theme::muted(),
            ),
        ]),
    );
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_selector(frame, layout[0], app);
    render_detail(frame, layout[1], app);
}

fn render_selector(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.settings_pane == SettingsPane::Menu;
    let mut spans = vec![Span::styled(
        if focused { "> " } else { "  " },
        theme::accent(),
    )];
    for section in SettingsSection::ALL {
        spans.push(Span::styled(
            format!(" {} ", section.label(app.locale)),
            if section == app.settings_section {
                theme::selected_label()
            } else {
                theme::text()
            },
        ));
        spans.push(Span::raw("  "));
    }
    let selector = Paragraph::new(Line::from(spans)).style(theme::text());
    frame.render_widget(selector, area);
}

fn render_detail(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let lines = match app.settings_section {
        SettingsSection::Status => status_lines(app),
        SettingsSection::General => general_lines(app),
        SettingsSection::McpAggregate => mcp_aggregate_lines(app),
        SettingsSection::Logging => logs::log_config_lines(app),
    };

    let detail = Paragraph::new(lines)
        .block(widgets::chrome::panel_block(
            app.settings_section.label(app.locale),
            app.focus_area == FocusArea::ViewTable && app.settings_pane == SettingsPane::Detail,
        ))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, area);
}

fn status_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let stats = app.header_stats();

    vec![
        kv_line(
            app.locale,
            TextKey::SettingsRuntimeStatus,
            "ready".to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsInstallPath,
            app.install_path.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsMcpConfigPath,
            app.config_path.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsAppConfigPath,
            app.app_config_path.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsConfigExists,
            exists_label(app.locale, app.app_config_exists).to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsCacheStorage,
            app.cache_storage_label.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsNamespace,
            app.namespace.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsSource,
            app.source_label.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsServiceCount,
            format!(
                "total={} ready={} not_ready={} unknown={}",
                stats.total, stats.ready, stats.not_ready, stats.unknown
            ),
        ),
    ]
}

fn mcp_aggregate_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let status = if app.mcp_aggregate_running {
        format!(
            "running pid={}",
            app.mcp_aggregate_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string())
        )
    } else {
        "stopped".to_string()
    };
    let endpoint = format!("http://127.0.0.1:{}/mcp", app.mcp_aggregate_port);
    let action = if app.mcp_aggregate_transport == "streamable-http" {
        "t: transport, Enter: start / stop, r: refresh"
    } else {
        "t: switch to HTTP; stdio is started by the MCP client"
    };
    vec![
        kv_line(app.locale, TextKey::SettingsRuntimeStatus, status),
        Line::from(vec![
            Span::styled("transport: ", theme::muted()),
            Span::styled(app.mcp_aggregate_transport.clone(), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("port: ", theme::muted()),
            Span::styled(app.mcp_aggregate_port.to_string(), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("endpoint: ", theme::muted()),
            Span::styled(endpoint, theme::text()),
        ]),
        Line::from(Span::styled(action, theme::muted())),
    ]
}

fn general_lines(app: &TuiApp) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("> ", theme::accent()),
            Span::styled(
                format!("{}: ", i18n::text(app.locale, TextKey::SettingsLocale)),
                theme::muted(),
            ),
            Span::styled(locale_label(app.locale), theme::selected_label()),
            Span::styled("  Enter", theme::muted()),
        ]),
        kv_line(
            app.locale,
            TextKey::SettingsLocaleSource,
            "tui --locale / default".to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsConfigFile,
            app.app_config_path.clone(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsConfigExists,
            exists_label(app.locale, app.app_config_exists).to_string(),
        ),
    ]
}

fn kv_line(locale: Locale, key: TextKey, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{}: ", i18n::text(locale, key)), theme::muted()),
        Span::styled(value, theme::text()),
    ])
}

fn focus_prefix(focused: bool) -> Span<'static> {
    Span::styled(if focused { "> " } else { "  " }, theme::accent())
}

fn exists_label(locale: Locale, exists: bool) -> &'static str {
    match (locale, exists) {
        (Locale::ZhCn, true) => "存在",
        (Locale::ZhCn, false) => "未创建",
        (Locale::EnUs, true) => "exists",
        (Locale::EnUs, false) => "missing",
    }
}

fn locale_label(locale: Locale) -> String {
    match locale {
        Locale::ZhCn => "zh-cn".to_string(),
        Locale::EnUs => "en-us".to_string(),
    }
}
