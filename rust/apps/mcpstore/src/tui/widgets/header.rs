use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const BANNER: &str = "\
███    ███  ██████  ███████  ██████  ████████  ██████  ██████  ███████
████  ████ ██      ██    ██ ██          ██    ██    ██ ██   ██ ██
██ ████ ██ ██      ███████  ██████      ██    ██    ██ ██████  █████
██  ██  ██ ██      ██           ██      ██    ██    ██ ██  ██  ██
██      ██  ██████ ██      ██████       ██     ██████  ██   ██ ███████";

pub const BANNER_HEIGHT: u16 = 6;

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
    BANNER_HEIGHT + 1
}

pub fn render(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let block = Block::default().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    render_banner(frame, layout[0]);
    render_stats(frame, layout[1], stats);
}

fn render_banner(frame: &mut Frame, area: Rect) {
    let area = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let mut banner_lines: Vec<Line> = vec![Line::from(Span::styled(
        "字符画",
        Style::default()
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ))];
    banner_lines.extend(BANNER.lines().map(|line| {
        Line::from(vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])
    }));

    let banner_height = banner_lines.len() as u16;
    let padding = area.height.saturating_sub(banner_height) / 2;

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(padding),
            Constraint::Length(banner_height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area)[1];

    let banner = Paragraph::new(banner_lines).alignment(Alignment::Left);
    frame.render_widget(banner, inner);
}

fn render_stats(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let text = vec![
        Line::from(Span::styled(
            "状态总览",
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![Span::styled(
            "MCPStore",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
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
        horizontal: 1,
        vertical: 0,
    });
    frame.render_widget(stats_block, area);

    let stats_widget = Paragraph::new(text).alignment(Alignment::Left);
    frame.render_widget(stats_widget, inner);
}
