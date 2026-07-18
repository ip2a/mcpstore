use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::{SelectedDetail, ToolSummary};
use crate::tui::i18n::{self, Locale, TextKey};
use crate::tui::{layout, theme};

pub fn render_confirm(frame: &mut Frame, title: &str, lines: Vec<Line<'static>>) {
    render_lines(
        frame,
        title,
        lines,
        layout::CONFIRM_DIALOG_PERCENT_X,
        layout::CONFIRM_DIALOG_PERCENT_Y,
    );
}

pub fn render_input(frame: &mut Frame, title: &str, value: &str, hint: &str) {
    let lines = vec![
        Line::from(Span::styled(title.to_string(), theme::accent_bold())),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", theme::accent()),
            Span::styled(value.to_string(), theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled(hint.to_string(), theme::muted())),
    ];
    render_lines(
        frame,
        title,
        lines,
        layout::INPUT_DIALOG_PERCENT_X,
        layout::INPUT_DIALOG_PERCENT_Y,
    );
}

pub fn render_loading(frame: &mut Frame, title: &str, message: &str) {
    let lines = vec![
        Line::from(Span::styled(title.to_string(), theme::accent_bold())),
        Line::from(""),
        Line::from(Span::styled(message.to_string(), theme::text())),
    ];
    render_lines(
        frame,
        title,
        lines,
        layout::LOADING_DIALOG_PERCENT_X,
        layout::LOADING_DIALOG_PERCENT_Y,
    );
}

pub fn render_select(
    frame: &mut Frame,
    locale: Locale,
    title: &str,
    options: &[String],
    selected: usize,
) {
    let mut lines = vec![
        Line::from(Span::styled(title.to_string(), theme::accent_bold())),
        Line::from(""),
    ];
    for (index, option) in options.iter().enumerate() {
        let active = index == selected;
        lines.push(Line::from(vec![
            Span::styled(if active { "> " } else { "  " }, theme::accent()),
            Span::styled(
                option.clone(),
                if active {
                    theme::selected_label()
                } else {
                    theme::text()
                },
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::text(locale, TextKey::SelectModalHint),
        theme::muted(),
    )));

    render_lines(
        frame,
        title,
        lines,
        layout::CONFIRM_DIALOG_PERCENT_X,
        layout::CONFIRM_DIALOG_PERCENT_Y,
    );
}

pub fn render_service_detail(frame: &mut Frame, locale: Locale, detail: &SelectedDetail) {
    let mut lines = vec![
        Line::from(Span::styled(detail.title.clone(), theme::accent_bold())),
        Line::from(""),
        kv_line(i18n::text(locale, TextKey::Transport), &detail.transport),
        kv_line(i18n::text(locale, TextKey::Endpoint), &detail.endpoint),
        kv_line(i18n::text(locale, TextKey::Scope), &detail.scope),
        kv_line(i18n::text(locale, TextKey::Added), &detail.added_time),
        kv_line("Readiness", &detail.readiness),
        kv_line("Phase", &detail.phase),
        kv_line(i18n::text(locale, TextKey::Health), &detail.health),
        kv_line("Recovery", &detail.recovery),
        kv_line(i18n::text(locale, TextKey::Latency), &detail.latency),
        kv_line(i18n::text(locale, TextKey::Retry), &detail.retry_time),
        kv_line(i18n::text(locale, TextKey::Error), &detail.error_message),
        Line::from(""),
        Line::from(Span::styled(
            i18n::text(locale, TextKey::Tools),
            theme::field_label(),
        )),
    ];

    if detail.tools.is_empty() {
        lines.push(Line::from(Span::styled("-", theme::muted())));
    } else {
        for tool in detail.tools.iter().take(12) {
            lines.push(Line::from(vec![
                Span::styled("- ", theme::accent()),
                Span::styled(tool.clone(), theme::text()),
            ]));
        }
        if detail.tools.len() > 12 {
            lines.push(Line::from(Span::styled(
                format!("... {} more", detail.tools.len() - 12),
                theme::muted(),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::text(locale, TextKey::ModalCloseHint),
        theme::muted(),
    )));

    render_lines(
        frame,
        i18n::text(locale, TextKey::ServiceDetail),
        lines,
        72,
        62,
    );
}

pub fn render_tool_detail(
    frame: &mut Frame,
    locale: Locale,
    service_name: &str,
    tool: &ToolSummary,
    test_args: &str,
    test_result: &[String],
) {
    let mut lines = vec![
        Line::from(Span::styled(tool.name.clone(), theme::accent_bold())),
        Line::from(""),
        kv_line(i18n::text(locale, TextKey::Service), service_name),
        kv_line(
            i18n::text(locale, TextKey::Description),
            if tool.description.trim().is_empty() {
                "-"
            } else {
                &tool.description
            },
        ),
        Line::from(""),
        Line::from(Span::styled(
            i18n::text(locale, TextKey::InputSchema),
            theme::field_label(),
        )),
    ];

    let schema =
        serde_json::to_string_pretty(&tool.input_schema).unwrap_or_else(|_| "{}".to_string());
    for line in schema.lines().take(18) {
        lines.push(Line::from(Span::styled(line.to_string(), theme::text())));
    }
    if schema.lines().count() > 18 {
        lines.push(Line::from(Span::styled("...", theme::muted())));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::text(locale, TextKey::Test),
        theme::field_label(),
    )));
    lines.push(kv_line(i18n::text(locale, TextKey::Args), test_args));
    if test_result.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("{}: -", i18n::text(locale, TextKey::Result)),
            theme::muted(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("{}:", i18n::text(locale, TextKey::Result)),
            theme::muted(),
        )));
        for line in test_result.iter().take(8) {
            lines.push(Line::from(Span::styled(line.clone(), theme::text())));
        }
        if test_result.len() > 8 {
            lines.push(Line::from(Span::styled(
                format!("... {} more", test_result.len() - 8),
                theme::muted(),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::text(locale, TextKey::TestToolKeyHint),
        theme::muted(),
    )));

    render_lines(
        frame,
        i18n::text(locale, TextKey::ToolDetail),
        lines,
        78,
        72,
    );
}

fn kv_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), theme::muted()),
        Span::styled(value.to_string(), theme::text()),
    ])
}

fn render_lines(
    frame: &mut Frame,
    title: &str,
    lines: Vec<Line<'static>>,
    percent_x: u16,
    percent_y: u16,
) {
    render_overlay(frame);

    let area = centered_rect(percent_x, percent_y, frame.area());
    frame.render_widget(Clear, area);

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::accent())
                .style(theme::modal_surface())
                .title(title.to_string()),
        )
        .alignment(Alignment::Left)
        .style(theme::modal_surface())
        .wrap(Wrap { trim: true });
    frame.render_widget(content, area);
}

fn render_overlay(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let overlay = Paragraph::new("").style(theme::modal_overlay());
    frame.render_widget(overlay, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
