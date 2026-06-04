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
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(50), Constraint::Min(30)])
        .split(area);

    render_banner(frame, layout[0]);
    render_stats(frame, layout[1], stats);
}

fn render_banner(frame: &mut Frame, area: Rect) {
    let lines: Vec<Line> = BANNER
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

    let banner = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(banner, area);
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
