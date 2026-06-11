use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{ContentPane, FocusArea, TuiApp},
    i18n::{self, TextKey},
    layout as tui_layout, theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let line = Line::from(vec![
        Span::styled(
            if app.focus_area == FocusArea::ViewFilter {
                "> "
            } else {
                "  "
            },
            theme::accent(),
        ),
        Span::styled("Agent列表", theme::field_label()),
        Span::raw("  "),
        Span::styled("r 刷新  e 选择Agent  a 授权服务  u 解除授权", theme::text()),
    ]);
    widgets::chrome::render_control_bar(frame, area, app, line);
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tui_layout::CONTENT_MENU_PERCENT),
            Constraint::Percentage(tui_layout::CONTENT_BODY_PERCENT),
        ])
        .split(area);

    render_agent_menu(frame, layout[0], app);
    render_agent_detail(frame, layout[1], app);
}

fn render_agent_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.agent_pane == ContentPane::Menu;
    let mut items = app
        .agents
        .iter()
        .enumerate()
        .map(|(index, agent)| {
            let selected = index == app.selected_agent;
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(if selected { "> " } else { "  " }, theme::accent()),
                    Span::styled(
                        agent.id.clone(),
                        if selected {
                            theme::menu_selected()
                        } else {
                            theme::text()
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("services={}", agent.services.len()), theme::muted()),
                ]),
            ])
            .style(if selected {
                theme::menu_selected()
            } else {
                theme::text()
            })
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        items.push(ListItem::new(Line::from("暂无 Agent，按 e 输入 Agent ID")));
    }

    let menu = List::new(items)
        .block(widgets::chrome::panel_block(" Agent列表 ", focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(menu, area);
}

fn render_agent_detail(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.agent_pane == ContentPane::Body;
    let title = format!(
        "{} / {}",
        i18n::text(app.locale, TextKey::ContentRegion),
        i18n::text(app.locale, TextKey::NavAgents)
    );

    let Some(agent) = app.current_agent() else {
        let body =
            Paragraph::new("暂无 Agent。按 e 输入 Agent ID，或在添加服务时选择 agent 作用域。")
                .block(widgets::chrome::panel_block(title, focused))
                .style(theme::text())
                .wrap(Wrap { trim: true });
        frame.render_widget(body, area);
        return;
    };

    let mut items = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Agent: ", theme::muted()),
            Span::styled(agent.id.clone(), theme::field_label()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("服务数量: ", theme::muted()),
            Span::styled(agent.services.len().to_string(), theme::text()),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(Span::styled("授权服务", theme::field_label()))),
    ];

    if agent.services.is_empty() {
        items.push(ListItem::new(Line::from(
            "  暂无授权服务，按 a 输入服务名授权。",
        )));
    } else {
        for (index, service) in agent.services.iter().enumerate() {
            let selected = focused && index == app.selected_agent_service;
            items.push(
                ListItem::new(Line::from(vec![
                    Span::styled(if selected { "> " } else { "  " }, theme::accent()),
                    Span::styled(
                        service.clone(),
                        if selected {
                            theme::field_selected()
                        } else {
                            theme::text()
                        },
                    ),
                    Span::raw("  "),
                    Span::styled("u 解除授权", theme::muted()),
                ]))
                .style(if selected {
                    theme::field_selected()
                } else {
                    theme::text()
                }),
            );
        }
    }

    let list = List::new(items)
        .block(widgets::chrome::panel_block(title, focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(list, area);
}
