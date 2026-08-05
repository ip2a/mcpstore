use std::collections::BTreeSet;

use mcpstore::state::ReadinessStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{ContentPane, FocusArea, ToolFilterTab, ToolSummary, TuiApp},
    i18n::{self, TextKey},
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    widgets::chrome::render_control_bar(
        frame,
        area,
        app,
        Line::from(vec![
            Span::styled(" Tools ", theme::field_label()),
            Span::styled(
                "h/l Scope  j/k Move  r Refresh  Enter Details  t Test  q Quit",
                theme::muted(),
            ),
        ]),
    );
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_selector(frame, layout[0], app);
    render_tool_list(frame, layout[1], app);
}

fn render_selector(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.tool_pane == ContentPane::Menu;
    let mut spans = vec![Span::styled(
        if focused { "> " } else { "  " },
        theme::accent(),
    )];
    for tab in ToolFilterTab::ALL {
        spans.push(Span::styled(
            format!(" {} ", tab.label()),
            if tab == app.tool_filter {
                theme::tab_selected()
            } else {
                theme::text()
            },
        ));
        spans.push(Span::raw("  "));
    }
    if app.tool_filter != ToolFilterTab::All {
        for (index, service) in app.tool_services.iter().enumerate() {
            spans.push(Span::styled(
                format!(" {} ", service.name),
                if index == app.selected_tool_service {
                    theme::menu_selected()
                } else {
                    theme::text()
                },
            ));
            spans.push(Span::raw(" "));
        }
    }
    let selector = Paragraph::new(Line::from(spans)).style(theme::text());
    frame.render_widget(selector, area);
}

fn render_tool_list(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.tool_pane == ContentPane::Body;
    let service_name = if app.tool_filter == ToolFilterTab::All {
        "全部服务"
    } else {
        app.current_tool_service_name().unwrap_or("-")
    };
    let title = format!(
        "{} / {} / {}",
        i18n::text(app.locale, TextKey::ContentRegion),
        i18n::text(app.locale, TextKey::NavTools),
        service_name
    );

    if app.tool_filter != ToolFilterTab::All && app.current_tool_service().is_none() {
        let body = Paragraph::new("当前工具分类下没有服务。")
            .block(widgets::chrome::panel_block(title, focused))
            .style(theme::text())
            .wrap(Wrap { trim: true });
        frame.render_widget(body, area);
        return;
    }

    if app.service_tools.is_empty() {
        if app.tool_filter == ToolFilterTab::All {
            let body = Paragraph::new("还没有读取到工具。按 r 会连接服务并读取全局工具列表。")
                .block(widgets::chrome::panel_block(title, focused))
                .style(theme::text())
                .wrap(Wrap { trim: true });
            frame.render_widget(body, area);
            return;
        }
        let service = app.current_tool_service().expect("checked above");
        let status = format_readiness(service.readiness);
        let body = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Service: ", theme::muted()),
                Span::styled(service.name.clone(), theme::field_label()),
            ]),
            Line::from(vec![
                Span::styled("Transport: ", theme::muted()),
                Span::styled(service.transport.clone(), theme::text()),
                Span::raw("  "),
                Span::styled("Status: ", theme::muted()),
                Span::styled(status, theme::text()),
            ]),
            Line::from(""),
            Line::from("还没有读取到工具。按 r 会连接该服务并读取工具列表。"),
        ])
        .block(widgets::chrome::panel_block(title, focused))
        .style(theme::text())
        .wrap(Wrap { trim: true });
        frame.render_widget(body, area);
        return;
    }

    let items = app
        .service_tools
        .iter()
        .enumerate()
        .map(|(index, tool)| tool_item(index == app.selected_tool, tool))
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(widgets::chrome::panel_block(title, focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(list, area);
}

fn tool_item(selected: bool, tool: &ToolSummary) -> ListItem<'static> {
    let params = schema_summary(&tool.input_schema);
    let param_label = if params.is_empty() {
        "无参数".to_string()
    } else {
        format!("{} 个参数: {}", params.len(), params.join(", "))
    };
    let desc = if tool.description.trim().is_empty() {
        "无描述".to_string()
    } else {
        truncate_chars(&tool.description, 120)
    };

    ListItem::new(vec![
        Line::from(vec![
            Span::styled(if selected { "> " } else { "  " }, theme::accent()),
            Span::styled(
                tool.name.clone(),
                if selected {
                    theme::field_selected()
                } else {
                    theme::field_label()
                },
            ),
            Span::raw("  "),
            Span::styled("Enter 详情  t 测试", theme::muted()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(param_label, theme::text()),
            Span::raw("  "),
            Span::styled(format!("service={}", tool.service_name), theme::muted()),
        ]),
        Line::from(vec![Span::raw("  "), Span::styled(desc, theme::muted())]),
        Line::from(""),
    ])
    .style(if selected {
        theme::field_selected()
    } else {
        theme::text()
    })
}

fn schema_summary(schema: &serde_json::Value) -> Vec<String> {
    let required: BTreeSet<String> = schema
        .get("required")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let Some(props) = schema.get("properties").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut out = props
        .iter()
        .map(|(name, value)| {
            let ty = value
                .get("type")
                .and_then(|value| value.as_str())
                .or_else(|| value.get("enum").map(|_| "enum"))
                .unwrap_or("any");
            if required.contains(name) {
                format!("{name}:{ty}*")
            } else {
                format!("{name}:{ty}")
            }
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn format_readiness(status: ReadinessStatus) -> &'static str {
    match status {
        ReadinessStatus::Ready => "ready",
        ReadinessStatus::NotReady => "not_ready",
        ReadinessStatus::Unknown => "unknown",
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
