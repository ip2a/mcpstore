use ratatui::{
    layout::Rect,
    text::{Line, Span},
    Frame,
};

use crate::tui::{
    app::{ServiceManagementTab, TuiApp},
    pages::{add_service, services},
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let (title, hints) = match app.service_tab {
        ServiceManagementTab::Services => (
            " Services ",
            "j/k Move  Enter Tools  a Add  s Status  o Sort  Ctrl-F Search  q Quit",
        ),
        ServiceManagementTab::AddService => (
            " Add service ",
            "h/l Focus  j/k Field  Enter Edit  a Save  Esc Services  q Quit",
        ),
    };
    widgets::chrome::render_control_bar(
        frame,
        area,
        app,
        Line::from(vec![
            Span::styled(title, theme::field_label()),
            Span::styled(hints, theme::muted()),
        ]),
    );
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    match app.service_tab {
        ServiceManagementTab::Services => services::render_content(frame, area, app),
        ServiceManagementTab::AddService => add_service::render_content(frame, area, app),
    }
}
