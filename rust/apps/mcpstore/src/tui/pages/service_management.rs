use ratatui::{
    layout::Rect,
    text::{Line, Span},
    Frame,
};

use crate::tui::{
    app::{FocusArea, ServiceManagementTab, TuiApp},
    pages::{add_service, services},
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let mut spans = vec![Span::styled(
        if app.focus_area == FocusArea::ViewFilter {
            "> "
        } else {
            "  "
        },
        theme::accent(),
    )];

    for tab in ServiceManagementTab::ALL {
        let style = if tab == app.service_tab {
            theme::tab_selected()
        } else {
            theme::text()
        };
        spans.push(Span::styled(format!(" {} ", tab.label(app.locale)), style));
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled("h/l 切换", theme::text()));

    widgets::chrome::render_control_bar(frame, area, app, Line::from(spans));
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    match app.service_tab {
        ServiceManagementTab::Services => services::render_content(frame, area, app),
        ServiceManagementTab::AddService => add_service::render_content(frame, area, app),
    }
}
