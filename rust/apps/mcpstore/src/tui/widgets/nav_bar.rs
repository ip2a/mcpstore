use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::MainView;

pub fn render(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    active_view: MainView,
    focused: bool,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let tab_constraints =
        vec![Constraint::Ratio(1, MainView::ALL.len() as u32); MainView::ALL.len()];
    let tabs = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(tab_constraints)
        .split(layout[0]);

    for (index, view) in MainView::ALL.iter().enumerate() {
        let style = if *view == active_view {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::Black)
        };
        let tab = Paragraph::new(Line::from(Span::styled(view.label(), style)))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(tab, tabs[index]);
    }

    let label = "主导航 ";
    let separator = format!(
        "{label}{}",
        "═".repeat(layout[1].width.saturating_sub(label.len() as u16) as usize)
    );
    let line = Paragraph::new(Line::from(Span::styled(
        separator,
        if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Black)
        },
    )))
    .block(Block::default().borders(Borders::NONE));
    frame.render_widget(line, layout[1]);
}
