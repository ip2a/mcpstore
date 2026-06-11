use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{layout as tui_layout, theme};

const BANNER: &str = "\
███    ███  ██████  ███████  ██████  ████████  ██████  ██████  ███████
████  ████ ██      ██    ██ ██          ██    ██    ██ ██   ██ ██
██ ████ ██ ██      ███████  ██████      ██    ██    ██ ██████  █████
██  ██  ██ ██      ██           ██      ██    ██    ██ ██  ██  ██
██      ██  ██████ ██      ██████       ██     ██████  ██   ██ ███████";

pub const BANNER_HEIGHT: u16 = 5;

pub struct HeaderStats {
    pub total: usize,
    pub connected: usize,
    pub error: usize,
    pub connecting: usize,
    pub disconnected: usize,
    pub backend: String,
    pub namespace: String,
    pub config_path: String,
}

/// Compute header height based on terminal width.
pub fn header_height(_term_width: u16) -> u16 {
    BANNER_HEIGHT + tui_layout::HEADER_BORDER_HEIGHT
}

pub fn render(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let block = Block::default().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tui_layout::HEADER_BANNER_PERCENT),
            Constraint::Percentage(tui_layout::HEADER_STATUS_PERCENT),
        ])
        .split(inner);

    render_banner(frame, layout[0]);
    render_stats(frame, layout[1], stats);
}

fn render_banner(frame: &mut Frame, area: Rect) {
    // Banner region: left side of the top header.
    let banner_lines: Vec<Line> = BANNER
        .lines()
        .map(|line| Line::from(vec![Span::styled(line.to_string(), theme::accent_bold())]))
        .collect();

    let banner_height = banner_lines.len() as u16;
    let vertical_padding = area.height.saturating_sub(banner_height) / 2;
    let banner_width = banner_area_width();
    let horizontal_padding = area.width.saturating_sub(banner_width) / 2;

    let banner_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(horizontal_padding),
            Constraint::Length(banner_width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(area)[1];

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vertical_padding),
            Constraint::Length(banner_height.min(banner_area.height)),
            Constraint::Min(0),
        ])
        .split(banner_area)[1]
        .inner(Margin {
            horizontal: tui_layout::BANNER_HORIZONTAL_MARGIN,
            vertical: 0,
        });

    let banner = Paragraph::new(banner_lines).alignment(Alignment::Left);
    frame.render_widget(banner, inner);
}

fn banner_area_width() -> u16 {
    let text_width = BANNER
        .lines()
        .map(|line| line.chars().count() as u16)
        .max()
        .unwrap_or(0);
    text_width.saturating_add(tui_layout::BANNER_HORIZONTAL_MARGIN.saturating_mul(2))
}

fn render_stats(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    // Status summary region: right side of the top header.
    let text = vec![
        Line::from(vec![Span::styled("MCPStore", theme::accent_bold())]),
        Line::from(format!(
            "total={}  connected={}  error={}  connecting={}  disconnected={}",
            stats.total, stats.connected, stats.error, stats.connecting, stats.disconnected
        )),
        Line::from(format!(
            "backend={}  namespace={}",
            stats.backend, stats.namespace
        )),
        Line::from(format!("config={}", stats.config_path)),
    ];

    let stats_block = Block::default().borders(Borders::LEFT);
    let inner = stats_block.inner(area).inner(Margin {
        horizontal: tui_layout::STATUS_HORIZONTAL_MARGIN,
        vertical: 0,
    });
    frame.render_widget(stats_block, area);

    let stats_widget = Paragraph::new(text).alignment(Alignment::Left);
    frame.render_widget(stats_widget, inner);
}
