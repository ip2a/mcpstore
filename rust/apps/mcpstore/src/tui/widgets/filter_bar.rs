use mcpstore::state::ReadinessStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{
    i18n::{self, Locale, TextKey},
    layout as tui_layout, theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterStatus {
    All,
    Ready,
    NotReady,
    Unknown,
}

impl FilterStatus {
    pub fn label(&self, _locale: Locale) -> &'static str {
        match self {
            FilterStatus::All => "All",
            FilterStatus::Ready => "Ready",
            FilterStatus::NotReady => "Not ready",
            FilterStatus::Unknown => "Unknown",
        }
    }

    pub fn matches(&self, status: ReadinessStatus) -> bool {
        match self {
            FilterStatus::All => true,
            FilterStatus::Ready => status == ReadinessStatus::Ready,
            FilterStatus::NotReady => status == ReadinessStatus::NotReady,
            FilterStatus::Unknown => status == ReadinessStatus::Unknown,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            FilterStatus::All => FilterStatus::Ready,
            FilterStatus::Ready => FilterStatus::NotReady,
            FilterStatus::NotReady => FilterStatus::Unknown,
            FilterStatus::Unknown => FilterStatus::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            FilterStatus::All => FilterStatus::Unknown,
            FilterStatus::Ready => FilterStatus::All,
            FilterStatus::NotReady => FilterStatus::Ready,
            FilterStatus::Unknown => FilterStatus::NotReady,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Status,
    Tools,
}

impl SortBy {
    pub fn label_key(&self) -> TextKey {
        match self {
            SortBy::Name => TextKey::SortName,
            SortBy::Status => TextKey::SortStatus,
            SortBy::Tools => TextKey::SortTools,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }

    pub fn next(&self) -> Self {
        match self {
            SortBy::Name => SortBy::Status,
            SortBy::Status => SortBy::Tools,
            SortBy::Tools => SortBy::Name,
        }
    }
}

pub struct FilterBarState {
    pub active_status: FilterStatus,
    pub search_text: String,
    pub search_mode: bool,
    pub sort_by: SortBy,
    pub sort_asc: bool,
}

impl Default for FilterBarState {
    fn default() -> Self {
        Self {
            active_status: FilterStatus::All,
            search_text: String::new(),
            search_mode: false,
            sort_by: SortBy::Name,
            sort_asc: true,
        }
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &FilterBarState,
    focused: bool,
    locale: Locale,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(tui_layout::FILTER_STATUS_MIN_WIDTH),
            Constraint::Min(tui_layout::FILTER_SEARCH_MIN_WIDTH),
        ])
        .split(area);

    render_status_tabs(frame, layout[0], state, focused, locale);
    render_search_and_sort(frame, layout[1], state, locale);
}

fn render_status_tabs(
    frame: &mut Frame,
    area: Rect,
    state: &FilterBarState,
    focused: bool,
    locale: Locale,
) {
    let tabs = [
        FilterStatus::All,
        FilterStatus::Ready,
        FilterStatus::NotReady,
        FilterStatus::Unknown,
    ];

    let mut spans = vec![Span::styled(
        if focused { "> " } else { "  " },
        theme::accent(),
    )];

    spans.extend(tabs.iter().enumerate().flat_map(|(i, tab)| {
        let is_active = *tab == state.active_status;
        let style = if is_active {
            theme::selected_label()
        } else {
            theme::text()
        };
        let prefix = if i > 0 { "  " } else { "" };
        vec![
            Span::raw(prefix),
            Span::styled(format!("{} {}", i + 1, tab.label(locale)), style),
        ]
    }));

    let paragraph =
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::NONE));

    frame.render_widget(paragraph, area);
}

fn render_search_and_sort(frame: &mut Frame, area: Rect, state: &FilterBarState, locale: Locale) {
    let search_display = if state.search_mode {
        format!(
            "[{}: {}_]",
            i18n::text(locale, TextKey::SearchLabel),
            state.search_text
        )
    } else if state.search_text.is_empty() {
        format!("[{}]", i18n::text(locale, TextKey::SearchPrompt))
    } else {
        format!(
            "[{}: {}]",
            i18n::text(locale, TextKey::SearchLabel),
            state.search_text
        )
    };

    let sort_display = format!(
        "{}:{} {}",
        i18n::text(locale, TextKey::SortLabel),
        state.sort_by.label(locale),
        if state.sort_asc { "↑" } else { "↓" }
    );

    let style = if state.search_mode {
        theme::accent_bold()
    } else {
        theme::text()
    };

    let text = Line::from(vec![
        Span::styled(search_display, style),
        Span::raw("  "),
        Span::styled(sort_display, theme::text()),
    ]);

    let paragraph = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(paragraph, area);
}
