use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{FocusArea, SettingsPane, SettingsSection, TuiApp},
    i18n::{self, Locale, TextKey},
    layout as tui_layout,
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
            Span::styled(" ", theme::text()),
        ]),
    );
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tui_layout::SETTINGS_MENU_PERCENT),
            Constraint::Percentage(tui_layout::SETTINGS_DETAIL_PERCENT),
        ])
        .split(area);

    render_menu(frame, layout[0], app);
    render_detail(frame, layout[1], app);
}

fn render_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.settings_pane == SettingsPane::Menu;
    let items = SettingsSection::ALL.iter().map(|section| {
        let selected = *section == app.settings_section;
        let marker = if selected { "> " } else { "  " };
        let style = if selected {
            theme::selected_label()
        } else {
            theme::text()
        };
        ListItem::new(Line::from(vec![
            Span::styled(marker, theme::accent()),
            Span::styled(section.label(app.locale), style),
        ]))
    });

    let menu = List::new(items)
        .block(widgets::chrome::panel_block(
            format!(
                "{} / {}",
                i18n::text(app.locale, TextKey::ContentRegion),
                i18n::text(app.locale, TextKey::NavSettings)
            ),
            focused,
        ))
        .style(theme::text());
    frame.render_widget(menu, area);
}

fn render_detail(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let lines = match app.settings_section {
        SettingsSection::Status => status_lines(app),
        SettingsSection::General => general_lines(app),
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
    let manager = app.store.config_manager();
    let install_path = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "-".to_string());
    let stats = app.header_stats();

    vec![
        kv_line(
            app.locale,
            TextKey::SettingsRuntimeStatus,
            "ready".to_string(),
        ),
        kv_line(app.locale, TextKey::SettingsInstallPath, install_path),
        kv_line(
            app.locale,
            TextKey::SettingsMcpConfigPath,
            manager.mcp_path().display().to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsAppConfigPath,
            manager.app_config_path().display().to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsConfigExists,
            exists_label(app.locale, manager.app_config_exists()).to_string(),
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

fn general_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let manager = app.store.config_manager();
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
            manager.app_config_path().display().to_string(),
        ),
        kv_line(
            app.locale,
            TextKey::SettingsConfigExists,
            exists_label(app.locale, manager.app_config_exists()).to_string(),
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
