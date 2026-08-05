use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::{
    app::{ContentPane, FocusArea, ServiceListMenu, TuiApp},
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    super::service_management::render_control_bar(frame, area, app);
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let content = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);
    render_scope_selector(frame, content[0], app);
    widgets::filter_bar::render(
        frame,
        content[1],
        &app.filter,
        app.focus_area == FocusArea::ViewFilter,
        app.locale,
    );

    widgets::service_table::render(
        frame,
        content[2],
        &app.filtered_services,
        &mut app.table_state,
        app.focus_area == FocusArea::ViewTable && app.service_list_pane == ContentPane::Body,
        app.locale,
    );
}

fn render_scope_selector(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused =
        app.focus_area == FocusArea::ViewTable && app.service_list_pane == ContentPane::Menu;
    let mut spans = vec![
        Span::styled(if focused { "▸ " } else { "  " }, theme::accent()),
        Span::styled("Scope  ", theme::muted()),
    ];
    for item in ServiceListMenu::ALL {
        let selected = item == app.service_list_menu;
        spans.push(Span::styled(
            format!(" {} ", item.label()),
            if selected {
                theme::selected_label()
            } else {
                theme::text()
            },
        ));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
