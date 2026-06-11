use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding},
    Frame,
};

use crate::tui::{
    app::{ContentPane, FocusArea, ServiceListMenu, TuiApp},
    layout as tui_layout, theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    super::service_management::render_control_bar(frame, area, app);
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
    widgets::service_table::render(
        frame,
        layout[1],
        &app.filtered_services,
        &mut app.table_state,
        app.focus_area == FocusArea::ViewTable && app.service_list_pane == ContentPane::Body,
        app.locale,
    );
}

fn render_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused =
        app.focus_area == FocusArea::ViewTable && app.service_list_pane == ContentPane::Menu;
    let items = ServiceListMenu::ALL.iter().map(|item| {
        let selected = *item == app.service_list_menu;
        ListItem::new(Line::from(vec![
            Span::styled(if selected { "> " } else { "  " }, theme::accent()),
            Span::styled(
                item.label(),
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
        .block(widgets::chrome::panel_block(" 服务筛选 ", focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(menu, area);
}
