use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{app::TuiApp, theme};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp, left: Line<'static>) {
    let status_width = control_status_width(area.width, &app.status_message);
    let layout =
        Layout::horizontal([Constraint::Min(12), Constraint::Length(status_width)]).split(area);

    let left_bar = Paragraph::new(left).block(Block::default().borders(Borders::NONE));
    frame.render_widget(left_bar, layout[0]);

    let right_bar = Paragraph::new(truncate(
        &app.status_message,
        status_width.saturating_sub(1),
    ))
    .style(status_style(&app.status_message))
    .alignment(ratatui::layout::Alignment::Right)
    .block(Block::default().borders(Borders::NONE));
    frame.render_widget(right_bar, layout[1]);
}

pub fn panel_block(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused {
            theme::accent()
        } else {
            theme::text()
        })
        .title(title.into())
}

fn control_status_width(area_width: u16, message: &str) -> u16 {
    let message_width = message.chars().count() as u16 + 1;
    let max_width = (area_width / 2).max(24);
    message_width
        .min(max_width)
        .min(area_width.saturating_sub(8))
}

fn truncate(value: &str, limit: u16) -> String {
    if limit == 0 {
        return String::new();
    }

    let mut chars = value.chars();
    let head: String = chars.by_ref().take(limit as usize).collect();
    if chars.next().is_some() && limit > 3 {
        format!(
            "{}...",
            head.chars().take(limit as usize - 3).collect::<String>()
        )
    } else {
        head
    }
}

fn status_style(message: &str) -> Style {
    if message.starts_with("[错误]") || message.starts_with("[Error]") {
        theme::error()
    } else if message.starts_with("[警告]") || message.starts_with("[Warning]") {
        theme::warning()
    } else if message.starts_with("[成功]") || message.starts_with("[Success]") {
        theme::success()
    } else {
        theme::muted()
    }
}
