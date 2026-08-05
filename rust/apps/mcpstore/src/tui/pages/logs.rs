use mcpstore::state::ReadinessStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{FocusArea, LogsPane, LogsSection, TuiApp},
    i18n::{self, TextKey},
    theme, widgets,
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
        Span::styled(
            "h/l Focus  j/k Section  r Refresh  Enter Open  Esc Back  q Quit",
            theme::muted(),
        ),
    ]);
    widgets::chrome::render_control_bar(frame, area, app, line);
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_selector(frame, layout[0], app);
    render_body(frame, layout[1], app);
}

fn render_selector(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.logs_pane == LogsPane::Menu;
    let mut spans = vec![Span::styled(
        if focused { "> " } else { "  " },
        theme::accent(),
    )];
    for section in LogsSection::ALL {
        spans.push(Span::styled(
            format!(" {} ", section.label(app.locale)),
            if section == app.logs_section {
                theme::menu_selected()
            } else {
                theme::text()
            },
        ));
        spans.push(Span::raw("  "));
    }
    let selector = Paragraph::new(Line::from(spans)).style(theme::text());
    frame.render_widget(selector, area);
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
    let ready = app
        .all_services
        .iter()
        .filter(|service| service.readiness == ReadinessStatus::Ready)
        .count();
    let not_ready = app
        .all_services
        .iter()
        .filter(|service| service.readiness == ReadinessStatus::NotReady)
        .count();
    items.push(ListItem::new(Line::from(format!(
        "summary  total={total} ready={ready} not_ready={not_ready}"
    ))));

    for service in &app.all_services {
        items.push(ListItem::new(Line::from(format!(
            "{}  transport={}  readiness={:?} phase={:?} health={:?} endpoint={}",
            service.name,
            service.transport,
            service.readiness,
            service.phase,
            service.health,
            service.endpoint
        ))));
    }

    if let Some(detail) = app.selected_detail.as_ref() {
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(format!(
            "selected  {}  readiness={} phase={} health={} recovery={} latency={}",
            detail.title,
            detail.readiness,
            detail.phase,
            detail.health,
            detail.recovery,
            detail.latency
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
    app.log_config
        .iter()
        .map(|(key, value)| kv_line(app, *key, value.clone()))
        .collect()
}

fn kv_line(app: &TuiApp, key: TextKey, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{}: ", i18n::text(app.locale, key)), theme::muted()),
        Span::styled(value, theme::text()),
    ])
}
