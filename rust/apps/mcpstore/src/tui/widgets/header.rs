use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const BANNER: &str = r#"███    ███  ██████  ███████  ██████  ████████  ██████  ██████  ███████
████  ████ ██      ██    ██ ██          ██    ██    ██ ██   ██ ██
██ ████ ██ ██      ███████  ██████      ██    ██    ██ ██████  █████
██  ██  ██ ██      ██           ██      ██    ██    ██ ██  ██  ██
██      ██  ██████ ██      ██████       ██     ██████  ██   ██ ███████"#;

/// Display width of the ASCII art (each █ is fullwidth=2).
/// Longest line: 35 fullwidth chars + 33 spaces = 103 display columns.
pub const BANNER_DISPLAY_WIDTH: u16 = 103;
pub const BANNER_HEIGHT: u16 = 5;
pub const STATS_MIN_WIDTH: u16 = 40;

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
pub fn header_height(term_width: u16) -> u16 {
    let border = 1u16;
    if term_width >= BANNER_DISPLAY_WIDTH + STATS_MIN_WIDTH {
        // Side-by-side: banner height + bottom border
        BANNER_HEIGHT + border
    } else {
        // Stacked: banner + stats (4 lines) + bottom border
        BANNER_HEIGHT + 4 + border
    }
}

pub fn render(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let block = Block::default().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width >= BANNER_DISPLAY_WIDTH + STATS_MIN_WIDTH {
        render_side_by_side(frame, inner, stats);
    } else {
        render_stacked(frame, inner, stats);
    }
}

fn render_side_by_side(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let banner_width = BANNER_DISPLAY_WIDTH.min(area.width.saturating_sub(STATS_MIN_WIDTH));
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(banner_width), Constraint::Min(STATS_MIN_WIDTH)])
        .split(area);

    render_banner(frame, layout[0]);

    // Left border separates stats from banner in side-by-side mode
    let stats_block = Block::default().borders(Borders::LEFT);
    let stats_inner = stats_block.inner(layout[1]);
    frame.render_widget(stats_block, layout[1]);
    render_stats_content(frame, stats_inner, stats);
}

fn render_stacked(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(BANNER_HEIGHT), Constraint::Min(0)])
        .split(area);

    render_banner(frame, layout[0]);
    render_stats_content(frame, layout[1], stats);
}

fn render_banner(frame: &mut Frame, area: Rect) {
    let banner_lines: Vec<Line> = BANNER
        .lines()
        .map(|line| {
            Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])
        })
        .collect();

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

    let banner = Paragraph::new(banner_lines).alignment(Alignment::Center);
    frame.render_widget(banner, inner);
}

fn render_stats_content(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let text = vec![
        Line::from(vec![
            Span::styled("MCPStore", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!(
            "total={}  connected={}  error={}  connecting={}  disconnected={}",
            stats.total, stats.connected, stats.error, stats.connecting, stats.disconnected
        )),
        Line::from(format!("backend={}  namespace={}", stats.backend, stats.namespace)),
        Line::from(format!("config={}", stats.config_path)),
    ];

    let stats_widget = Paragraph::new(text).alignment(Alignment::Left);
    frame.render_widget(stats_widget, area);
}
