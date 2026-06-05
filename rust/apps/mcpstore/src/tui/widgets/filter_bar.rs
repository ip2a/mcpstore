use mcpstore::registry::ConnectionStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterStatus {
    All,
    Connected,
    Error,
    Disconnected,
    Connecting,
}

impl FilterStatus {
    pub fn label(&self) -> &'static str {
        match self {
            FilterStatus::All => "全部",
            FilterStatus::Connected => "已连接",
            FilterStatus::Error => "错误",
            FilterStatus::Disconnected => "断开",
            FilterStatus::Connecting => "连接中",
        }
    }

    pub fn matches(&self, status: ConnectionStatus) -> bool {
        match self {
            FilterStatus::All => true,
            FilterStatus::Connected => status == ConnectionStatus::Connected,
            FilterStatus::Error => status == ConnectionStatus::Error,
            FilterStatus::Disconnected => status == ConnectionStatus::Disconnected,
            FilterStatus::Connecting => status == ConnectionStatus::Connecting,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            FilterStatus::All => FilterStatus::Connected,
            FilterStatus::Connected => FilterStatus::Error,
            FilterStatus::Error => FilterStatus::Disconnected,
            FilterStatus::Disconnected => FilterStatus::Connecting,
            FilterStatus::Connecting => FilterStatus::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            FilterStatus::All => FilterStatus::Connecting,
            FilterStatus::Connected => FilterStatus::All,
            FilterStatus::Error => FilterStatus::Connected,
            FilterStatus::Disconnected => FilterStatus::Error,
            FilterStatus::Connecting => FilterStatus::Disconnected,
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
    pub fn label(&self) -> &'static str {
        match self {
            SortBy::Name => "名称",
            SortBy::Status => "状态",
            SortBy::Tools => "工具",
        }
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

pub fn render(frame: &mut Frame, area: Rect, state: &FilterBarState, focused: bool) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(40), Constraint::Min(30)])
        .split(area);

    render_status_tabs(frame, layout[0], state, focused);
    render_search_and_sort(frame, layout[1], state);
}

fn render_status_tabs(frame: &mut Frame, area: Rect, state: &FilterBarState, focused: bool) {
    let tabs = [
        FilterStatus::All,
        FilterStatus::Connected,
        FilterStatus::Error,
        FilterStatus::Disconnected,
        FilterStatus::Connecting,
    ];

    let mut spans = vec![Span::styled(
        if focused { "> " } else { "  " },
        Style::default().fg(Color::Cyan),
    )];

    spans.extend(tabs.iter().enumerate().flat_map(|(i, tab)| {
        let is_active = *tab == state.active_status;
        let style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::Black)
        };
        let prefix = if i > 0 { "  " } else { "" };
        vec![
            Span::raw(prefix),
            Span::styled(format!("{} {}", i + 1, tab.label()), style),
        ]
    }));

    let paragraph =
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::NONE));

    frame.render_widget(paragraph, area);
}

fn render_search_and_sort(frame: &mut Frame, area: Rect, state: &FilterBarState) {
    let search_display = if state.search_mode {
        format!("[搜索: {}_]", state.search_text)
    } else if state.search_text.is_empty() {
        "[按 / 搜索]".to_string()
    } else {
        format!("[搜索: {}]", state.search_text)
    };

    let sort_display = format!(
        "排序:{} {}",
        state.sort_by.label(),
        if state.sort_asc { "↑" } else { "↓" }
    );

    let style = if state.search_mode {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Black)
    };

    let text = Line::from(vec![
        Span::styled(search_display, style),
        Span::raw("  "),
        Span::styled(sort_display, Style::default().fg(Color::Black)),
    ]);

    let paragraph = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(paragraph, area);
}
