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

    let banner = Paragraph::new(banner_lines)
        .alignment(Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(banner, inner);
}

fn render_stats(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
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

    let stats_widget = Paragraph::new(text)
        .block(Block::default().borders(Borders::LEFT))
        .alignment(Alignment::Left);

    frame.render_widget(stats_widget, area);
}
