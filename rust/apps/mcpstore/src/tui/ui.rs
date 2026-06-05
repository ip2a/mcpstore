use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::app::{FocusArea, MainView, TuiApp};
use super::widgets;

pub fn draw(frame: &mut Frame, app: &mut TuiApp, _rt: &tokio::runtime::Runtime) {
    let term_width = frame.area().width;
    let header_h = widgets::header::header_height(term_width);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h), // Header dynamically sized for ASCII art
            Constraint::Length(2),        // Main navigation + divider
            Constraint::Min(10),          // Active view content
        ])
        .split(frame.area());

    widgets::header::render(frame, main_layout[0], &app.header_stats());
    widgets::nav_bar::render(
        frame,
        main_layout[1],
        app.active_view,
        app.focus_area == FocusArea::MainNav,
    );
    render_active_view(frame, main_layout[2], app);

    if app.pending_action.is_some() {
        render_confirm_dialog(frame);
    }
}

fn render_active_view(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Per-view filters/actions
            Constraint::Min(8),    // Per-view table/form
        ])
        .split(area);

    match app.active_view {
        MainView::Services => {
            widgets::filter_bar::render(
                frame,
                content_layout[0],
                &app.filter,
                app.focus_area == FocusArea::ViewFilter,
            );
            widgets::service_table::render(
                frame,
                content_layout[1],
                &app.filtered_services,
                &mut app.table_state,
                app.focus_area == FocusArea::ViewTable,
            );
        }
        MainView::Tools => {
            render_placeholder_view(frame, &content_layout, "工具列表", app.focus_area)
        }
        MainView::Agents => {
            render_placeholder_view(frame, &content_layout, "Agent列表", app.focus_area)
        }
        MainView::Logs => render_placeholder_view(frame, &content_layout, "日志", app.focus_area),
        MainView::Status => render_placeholder_view(frame, &content_layout, "状态", app.focus_area),
        MainView::Settings => {
            render_placeholder_view(frame, &content_layout, "设置", app.focus_area)
        }
    }
}

fn render_placeholder_view(
    frame: &mut Frame,
    layout: &[Rect],
    title: &'static str,
    focus_area: FocusArea,
) {
    let filter = Paragraph::new(Line::from(vec![
        Span::styled(
            if focus_area == FocusArea::ViewFilter {
                "> "
            } else {
                "  "
            },
            Style::default().fg(Color::Cyan),
        ),
        Span::styled("筛选区", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled("待接入", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::NONE));
    frame.render_widget(filter, layout[0]);

    let body = Paragraph::new(Line::from(format!("{title} 表单区待接入")))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if focus_area == FocusArea::ViewTable {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Black)
                })
                .title(format!("内容区 / {title}")),
        )
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(body, layout[1]);
}

fn render_confirm_dialog(frame: &mut Frame) {
    let area = centered_rect(48, 20, frame.area());
    frame.render_widget(Clear, area);

    let text = Paragraph::new(vec![
        Line::from(Span::styled(
            "确认删除",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("删除会同步修改当前 store 配置与运行态缓存。"),
        Line::from("按 y 确认删除，按 n 或 Esc 取消。"),
    ])
    .block(Block::default().borders(Borders::ALL).title("危险操作"))
    .wrap(Wrap { trim: true });

    frame.render_widget(text, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
