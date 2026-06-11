use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    // Main navigation region: switches the active content view.
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let page_count = pages.len().max(1);
    let tab_constraints = vec![Constraint::Ratio(1, page_count as u32); page_count];
    let tabs = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(tab_constraints)
        .split(layout[0]);

    for (index, page) in pages.iter().enumerate() {
        let style = if page.id == active_view {
            theme::selected_label()
        } else {
            theme::text()
        };
        let tab = Paragraph::new(Line::from(Span::styled(page.title(locale), style)))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(tab, tabs[index]);
    }

    let separator = "═".repeat(layout[1].width as usize);
    let line = Paragraph::new(Line::from(Span::styled(
        separator,
        if focused {
            theme::accent()
        } else {
            theme::text()
        },
    )))
    .block(Block::default().borders(Borders::NONE));
    frame.render_widget(line, layout[1]);
}
