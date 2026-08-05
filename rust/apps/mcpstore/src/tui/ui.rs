use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::app::{FocusArea, TuiApp};
use crate::tui::i18n::{self, Locale, TextKey};
use crate::tui::{layout, pages, theme, widgets};

pub fn draw(frame: &mut Frame, app: &mut TuiApp) {
    if frame.area().width < 40 || frame.area().height < 16 {
        frame.render_widget(
            Paragraph::new("Terminal too small. Resize to at least 40x16."),
            frame.area(),
        );
        return;
    }
    let term_width = frame.area().width;
    let header_h = widgets::header::header_height(term_width);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Length(layout::MAIN_NAV_HEIGHT),
            Constraint::Length(layout::CONTROL_BAR_HEIGHT),
            Constraint::Min(layout::MAIN_CONTENT_MIN_HEIGHT),
        ])
        .split(frame.area());

    widgets::header::render(frame, main_layout[0], &app.header_stats());
    let visible_pages = pages::visible_pages();
    widgets::nav_bar::render(
        frame,
        main_layout[1],
        &visible_pages,
        app.active_view,
        app.focus_area == FocusArea::MainNav,
        app.locale,
    );
    pages::render_control_bar(frame, main_layout[2], app);
    pages::render_content(frame, main_layout[3], app);

    match &app.overlay {
        super::app::Overlay::Confirm(_) => render_confirm_dialog(frame, app.locale),
        super::app::Overlay::ServiceDetail => {
            if let Some(detail) = app.selected_detail.as_ref() {
                widgets::modal::render_service_detail(frame, app.locale, detail);
            }
        }
        super::app::Overlay::ToolDetail => {
            if let Some(tool) = app.current_tool() {
                widgets::modal::render_tool_detail(
                    frame,
                    app.locale,
                    &tool.service_name,
                    tool,
                    &app.tool_test_args,
                    &app.tool_test_result,
                );
            }
        }
        super::app::Overlay::Edit(modal) => {
            widgets::modal::render_input(frame, &modal.title, &modal.value, &modal.hint);
        }
        super::app::Overlay::Select(modal) => {
            widgets::modal::render_select(
                frame,
                app.locale,
                &modal.title,
                &modal.options,
                modal.selected,
            );
        }
        super::app::Overlay::Loading(modal) => {
            widgets::modal::render_loading(frame, &modal.title, &modal.message);
        }
        super::app::Overlay::None => {}
    }
}

fn render_confirm_dialog(frame: &mut Frame, locale: Locale) {
    widgets::modal::render_confirm(
        frame,
        i18n::text(locale, TextKey::DangerousOperation),
        vec![
            Line::from(Span::styled(
                i18n::text(locale, TextKey::ConfirmDelete),
                theme::danger(),
            )),
            Line::from(""),
            Line::from(i18n::text(locale, TextKey::DeleteConfirmDescription)),
            Line::from(i18n::text(locale, TextKey::DeleteConfirmHint)),
        ],
    );
}
