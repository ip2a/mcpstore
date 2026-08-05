use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::{app::MainView, i18n::Locale, pages::PageDescriptor, theme};

pub fn render(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    pages: &[PageDescriptor],
    active_view: MainView,
    focused: bool,
    locale: Locale,
) {
    let page_count = pages.len().max(1);
    let tab_constraints = vec![Constraint::Ratio(1, page_count as u32); page_count];
    let tabs = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(tab_constraints)
        .split(area);
    let compact = area.width < 100;

    for (index, page) in pages.iter().enumerate() {
        let style = if page.id == active_view {
            theme::selected_label()
        } else {
            theme::text()
        };
        let label = if compact {
            format!(
                "{} {}",
                index + 1,
                truncate(page.title(locale), tabs[index].width.saturating_sub(2))
            )
        } else {
            page.title(locale).to_string()
        };
        let marker = if page.id == active_view { "▸ " } else { "  " };
        let tab = Paragraph::new(Line::from(vec![
            Span::styled(marker, if focused { theme::accent() } else { style }),
            Span::styled(label, style),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(tab, tabs[index]);
    }
}

fn truncate(value: &str, width: u16) -> String {
    value.chars().take(width as usize).collect()
}
