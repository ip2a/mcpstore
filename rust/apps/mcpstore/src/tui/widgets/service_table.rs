use mcpstore::state::{HealthState, ReadinessStatus, RecoveryState, RuntimePhase, ServiceState};
use mcpstore::{InstanceId, ScopeRef};
use ratatui::{
    style::Style,
    widgets::{Row, Table, TableState},
    Frame,
};

use super::filter_bar::{FilterBarState, SortBy};
use crate::tui::{
    i18n::{self, Locale, TextKey},
    layout as tui_layout, theme, widgets,
};

#[derive(Clone)]
pub struct ServiceSummary {
    pub instance_id: InstanceId,
    pub name: String,
    pub scope: ScopeRef,
    pub transport: String,
    pub endpoint: String,
    pub readiness: ReadinessStatus,
    pub phase: RuntimePhase,
    pub health: HealthState,
    pub recovery: RecoveryState,
    pub tools: usize,
    pub added_time: i64,
}

impl ServiceSummary {
    pub fn new(value: mcpstore::registry::ServiceInstance, state: ServiceState) -> Self {
        let endpoint = value
            .url
            .clone()
            .or(value.command.clone())
            .unwrap_or_else(|| "-".to_string());

        Self {
            instance_id: value.instance_id,
            name: value.service_name,
            scope: value.scope,
            transport: value.transport,
            endpoint,
            readiness: state.readiness.status,
            phase: state.phase,
            health: state.health,
            recovery: state.recovery,
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
        .filter(|s| filter.active_status.matches(s.readiness))
        .filter(|s| {
            if filter.search_text.is_empty() {
                return true;
            }
            let query = filter.search_text.to_lowercase();
            s.name.to_lowercase().contains(&query)
                || scope_label(&s.scope).to_lowercase().contains(&query)
                || s.transport.to_lowercase().contains(&query)
                || s.endpoint.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();

    result.sort_by(|a, b| {
        let ord = match filter.sort_by {
            SortBy::Name => a.name.cmp(&b.name),
            SortBy::Status => status_order(a.readiness).cmp(&status_order(b.readiness)),
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

fn status_order(status: ReadinessStatus) -> u8 {
    match status {
        ReadinessStatus::Ready => 0,
        ReadinessStatus::NotReady => 1,
        ReadinessStatus::Unknown => 2,
    }
}

pub fn render(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    services: &[ServiceSummary],
    table_state: &mut TableState,
    focused: bool,
    locale: Locale,
) {
    let header = Row::new(vec![
        i18n::text(locale, TextKey::TableName),
        i18n::text(locale, TextKey::TableScope),
        i18n::text(locale, TextKey::TableProtocol),
        i18n::text(locale, TextKey::TableStatus),
        i18n::text(locale, TextKey::TableTools),
        i18n::text(locale, TextKey::TableActions),
    ])
    .style(theme::table_header())
    .height(1);

    let rows = services.iter().map(|service| {
        let scope = truncate_text(&scope_label(&service.scope), 10);

        Row::new(vec![
            truncate_text(&service.name, 22),
            scope,
            service.transport.clone(),
            format_readiness(service.readiness).to_string(),
            service.tools.to_string(),
            i18n::text(locale, TextKey::ServiceRowActions).to_string(),
        ])
        .style(status_style(service.readiness))
    });

    let row_highlight_style = if focused {
        theme::table_row_highlight()
    } else {
        Style::default()
    };

    let table = Table::new(rows, tui_layout::service_table_widths())
        .header(header)
        .block(widgets::chrome::panel_block(
            format!(
                "{} / {}",
                i18n::text(locale, TextKey::ContentRegion),
                i18n::text(locale, TextKey::NavServices)
            ),
            focused,
        ))
        .row_highlight_style(row_highlight_style)
        .highlight_symbol(if focused { ">>> " } else { "    " });

    frame.render_stateful_widget(table, area, table_state);
}

fn scope_label(scope: &ScopeRef) -> String {
    match scope {
        ScopeRef::Store => "store".to_string(),
        ScopeRef::Agent { agent_id } => agent_id.clone(),
    }
}

fn status_style(status: ReadinessStatus) -> Style {
    match status {
        ReadinessStatus::Ready => theme::success(),
        ReadinessStatus::NotReady => theme::error(),
        ReadinessStatus::Unknown => theme::muted(),
    }
}

fn format_readiness(status: ReadinessStatus) -> &'static str {
    match status {
        ReadinessStatus::Ready => "ready",
        ReadinessStatus::NotReady => "not_ready",
        ReadinessStatus::Unknown => "unknown",
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
