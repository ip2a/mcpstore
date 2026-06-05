use mcpstore::registry::ConnectionStatus;
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

use super::filter_bar::{FilterBarState, SortBy};

#[derive(Clone)]
pub struct ServiceSummary {
    pub name: String,
    pub original_name: String,
    pub agent_id: String,
    pub transport: String,
    pub endpoint: String,
    pub status: ConnectionStatus,
    pub tools: usize,
    pub added_time: i64,
}

impl From<mcpstore::registry::ServiceEntry> for ServiceSummary {
    fn from(value: mcpstore::registry::ServiceEntry) -> Self {
        let endpoint = value
            .url
            .clone()
            .or(value.command.clone())
            .unwrap_or_else(|| "-".to_string());

        Self {
            name: value.name,
            original_name: value.original_name,
            agent_id: value.agent_id,
            transport: value.transport,
            endpoint,
            status: value.status,
            tools: value.tools.len(),
            added_time: value.added_time,
        }
    }
}

pub fn filter_and_sort(
    services: &[ServiceSummary],
    filter: &FilterBarState,
) -> Vec<ServiceSummary> {
    let mut result: Vec<ServiceSummary> = services
        .iter()
        .filter(|s| filter.active_status.matches(s.status))
        .filter(|s| {
            if filter.search_text.is_empty() {
                return true;
            }
            let query = filter.search_text.to_lowercase();
            s.name.to_lowercase().contains(&query)
                || s.original_name.to_lowercase().contains(&query)
                || s.agent_id.to_lowercase().contains(&query)
                || s.transport.to_lowercase().contains(&query)
                || s.endpoint.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();

    result.sort_by(|a, b| {
        let ord = match filter.sort_by {
            SortBy::Name => a.name.cmp(&b.name),
            SortBy::Status => status_order(a.status).cmp(&status_order(b.status)),
            SortBy::Tools => a.tools.cmp(&b.tools),
        };
        if filter.sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    result
}

fn status_order(status: ConnectionStatus) -> u8 {
    match status {
        ConnectionStatus::Connected => 0,
        ConnectionStatus::Connecting => 1,
        ConnectionStatus::Error => 2,
        ConnectionStatus::Disconnected => 3,
    }
}

pub fn render(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    services: &[ServiceSummary],
    table_state: &mut TableState,
    focused: bool,
) {
    let header = Row::new(vec!["名称", "作用域", "协议", "状态", "工具", "操作"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows = services.iter().map(|service| {
        let scope = if service.agent_id == "global_agent_store" {
            "store".to_string()
        } else {
            truncate_text(&service.agent_id, 10)
        };

        Row::new(vec![
            truncate_text(&service.name, 24),
            scope,
            truncate_text(&service.transport, 8),
            format_connection_status(service.status).to_string(),
            service.tools.to_string(),
            "c:连 d:断 x:启 D:删".to_string(),
        ])
        .style(status_style(service.status))
    });

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Black)
    };

    let row_highlight_style = if focused {
        Style::default()
            .bg(Color::Rgb(32, 42, 54))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Min(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("内容区 / 服务列表"),
    )
    .row_highlight_style(row_highlight_style)
    .highlight_symbol(if focused { ">>> " } else { "    " });

    frame.render_stateful_widget(table, area, table_state);
}

fn status_style(status: ConnectionStatus) -> Style {
    match status {
        ConnectionStatus::Connected => Style::default().fg(Color::Green),
        ConnectionStatus::Connecting => Style::default().fg(Color::Yellow),
        ConnectionStatus::Disconnected => Style::default().fg(Color::Gray),
        ConnectionStatus::Error => Style::default().fg(Color::Red),
    }
}

fn format_connection_status(status: ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connected => "connected",
        ConnectionStatus::Connecting => "connecting",
        ConnectionStatus::Disconnected => "disconnected",
        ConnectionStatus::Error => "error",
    }
}

fn truncate_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}
