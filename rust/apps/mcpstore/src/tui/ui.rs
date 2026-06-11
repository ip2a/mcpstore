use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    Frame,
};

use super::app::{FocusArea, TuiApp};
use super::{layout, pages, theme, widgets};

pub fn draw(frame: &mut Frame, app: &mut TuiApp, rt: &tokio::runtime::Runtime) {
    app.sync_status_history();
    if app.active_view == super::app::MainView::Logs {
        app.refresh_log_sources(rt);
    }
    if app.active_view == super::app::MainView::Agents {
        if let Err(error) = app.refresh_agents(rt) {
            app.status_message = format!("[错误] {error}");
        }
    }
    if app.active_view == super::app::MainView::Status {
        app.refresh_status_sources(rt);
    }
    let term_width = frame.area().width;
    let header_h = widgets::header::header_height(term_width);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h), // Header dynamically sized for ASCII art
            Constraint::Length(layout::MAIN_NAV_HEIGHT), // Main navigation + divider
            Constraint::Min(layout::MAIN_CONTENT_MIN_HEIGHT), // Active view content
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
    render_active_view(frame, main_layout[2], app);

    if app.pending_action.is_some() {
        render_confirm_dialog(frame);
    }

    if app.show_service_detail {
        if let Some(detail) = app.selected_detail.as_ref() {
            widgets::modal::render_service_detail(frame, detail);
        }
    }

    if app.show_tool_detail {
        if let (Some(service), Some(tool)) =
            (app.current_tool_call_service_name(), app.current_tool())
        {
            widgets::modal::render_tool_detail(
                frame,
                service,
                tool,
                &app.tool_test_args,
                &app.tool_test_result,
            );
        }
    }

    if let Some(modal) = app.edit_modal.as_ref() {
        widgets::modal::render_input(frame, &modal.title, &modal.value, &modal.hint);
    }

    if let Some(modal) = app.select_modal.as_ref() {
        widgets::modal::render_select(frame, &modal.title, &modal.options, modal.selected);
    }

    if let Some(modal) = app.loading_modal.as_ref() {
        widgets::modal::render_loading(frame, &modal.title, &modal.message);
    }
}

fn render_active_view(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    // Content region: contains the active view filter row and body.
    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(layout::CONTROL_BAR_HEIGHT), // Control bar region.
            Constraint::Length(layout::CONTROL_CONTENT_GAP_HEIGHT), // Gap between control bar and content.
            Constraint::Min(layout::VIEW_CONTENT_MIN_HEIGHT),       // Per-view table/form
        ])
        .split(area);

    pages::render_control_bar(frame, content_layout[0], app);
    pages::render_content(frame, content_layout[2], app);
}

fn render_confirm_dialog(frame: &mut Frame) {
    widgets::modal::render_confirm(
        frame,
        "危险操作",
        vec![
            Line::from(Span::styled("确认删除", theme::danger())),
            Line::from(""),
            Line::from("删除会同步修改当前 store 配置与运行态缓存。"),
            Line::from("按 y 确认删除，按 n 或 Esc 取消。"),
        ],
    );
}
