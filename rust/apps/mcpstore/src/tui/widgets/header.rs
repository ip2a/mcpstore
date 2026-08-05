use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::theme;

pub struct HeaderStats {
    pub total: usize,
    pub ready: usize,
    pub not_ready: usize,
    pub unknown: usize,
    pub cache_storage: String,
    pub namespace: String,
    pub config_path: String,
}

pub fn header_height(_term_width: u16) -> u16 {
    1
}

pub fn render(frame: &mut Frame, area: Rect, stats: &HeaderStats) {
    let summary = if area.width < 100 {
        format!(
            "{} svc · {} ready · {}",
            stats.total, stats.ready, stats.cache_storage
        )
    } else {
        format!(
            "services={}  ready={}  down={}  unknown={}  cache={}  ns={}",
            stats.total,
            stats.ready,
            stats.not_ready,
            stats.unknown,
            stats.cache_storage,
            stats.namespace
        )
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" MCPStore ", theme::accent_bold()),
            Span::styled("│ ", theme::muted()),
            Span::styled(summary, theme::text()),
        ])),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn header_is_always_one_line() {
        assert_eq!(header_height(120), 1);
        assert_eq!(header_height(60), 1);
    }

    #[test]
    fn compact_header_renders_at_narrow_width() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let stats = HeaderStats {
            total: 3,
            ready: 2,
            not_ready: 1,
            unknown: 0,
            cache_storage: "memory".to_string(),
            namespace: "default".to_string(),
            config_path: "/tmp/mcp.toml".to_string(),
        };

        terminal
            .draw(|frame| {
                let area = frame.area();
                render(frame, area, &stats);
            })
            .unwrap();
    }
}
