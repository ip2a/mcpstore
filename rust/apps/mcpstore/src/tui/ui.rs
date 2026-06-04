use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::app::TuiApp;
use super::widgets;

pub fn draw(frame: &mut Frame, app: &mut TuiApp, _rt: &tokio::runtime::Runtime) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Header (ASCII art + stats)
            Constraint::Length(1),  // Filter bar
            Constraint::Min(10),    // Service table
            Constraint::Length(2),  // Footer
        ])
        .split(frame.area());

    widgets::header::render(frame, main_layout[0], &app.header_stats());
    widgets::filter_bar::render(frame, main_layout[1], &app.filter);
    widgets::service_table::render(
        frame,
        main_layout[2],
        &app.filtered_services,
        &mut app.table_state,
    );
    render_footer(frame, main_layout[3], app);

    if app.pending_action.is_some() {
        render_confirm_dialog(frame);
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let key_hint = if app.filter.search_mode {
        "搜索模式: 输入关键词过滤服务，Backspace 删除，Esc 退出搜索"
    } else if app.pending_action.is_some() {
        "确认: y 确认 / n 或 Esc 取消"
    } else {
        "按键: ↑k ↓j 移动  gG 跳转  1-5 筛选  / 搜索  s 排序  S 排序方向  c 连接  d 断开  x 重启  D 删除  r 刷新  q 退出"
    };

    let footer = Paragraph::new(vec![
        Line::from(app.status_message.clone()),
        Line::from(Span::styled(key_hint, Style::default().fg(Color::Gray))),
    ])
    .block(Block::default().borders(Borders::ALL).title("状态"));

    frame.render_widget(footer, area);
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
