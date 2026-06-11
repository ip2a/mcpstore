use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{ContentPane, FocusArea, StatusSection, TuiApp},
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
        Span::styled("状态", theme::field_label()),
        Span::raw("  "),
        Span::styled("r 刷新  Enter 进入  Esc 返回", theme::text()),
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

    render_menu(frame, layout[0], app);
    render_body(frame, layout[1], app);
}

fn render_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.status_pane == ContentPane::Menu;
    let items = StatusSection::ALL.iter().map(|section| {
        let selected = *section == app.status_section;
        ListItem::new(Line::from(vec![
            Span::styled(if selected { "> " } else { "  " }, theme::accent()),
            Span::styled(
                section.label(),
                if selected {
                    theme::menu_selected()
                } else {
                    theme::text()
                },
            ),
        ]))
        .style(if selected {
            theme::menu_selected()
        } else {
            theme::text()
        })
    });

    let menu = List::new(items)
        .block(widgets::chrome::panel_block(" 状态模块 ", focused).padding(Padding::horizontal(1)))
        .style(theme::text());
    frame.render_widget(menu, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused = app.focus_area == FocusArea::ViewTable && app.status_pane == ContentPane::Body;
    let title = format!(
        "{} / {} / {}",
        i18n::text(app.locale, TextKey::ContentRegion),
        i18n::text(app.locale, TextKey::NavStatus),
        app.status_section.label()
    );
    let lines = match app.status_section {
        StatusSection::Overview => overview_lines(app),
        StatusSection::Cache => app
            .status_cache_lines
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect(),
        StatusSection::Events => app
            .status_event_lines
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect(),
        StatusSection::Capabilities => capability_lines(app),
    };

    let body = Paragraph::new(lines)
        .block(widgets::chrome::panel_block(title, focused).padding(Padding::horizontal(1)))
        .style(theme::text())
        .wrap(Wrap { trim: true });
    frame.render_widget(body, area);
}

fn overview_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let stats = app.header_stats();
    vec![
        kv("运行状态", "ready".to_string()),
        kv("配置来源", app.source_label.clone()),
        kv("缓存后端", app.backend_label.clone()),
        kv("命名空间", app.namespace.clone()),
        kv("MCP配置", app.config_path.clone()),
        kv("服务总数", stats.total.to_string()),
        kv("已连接", stats.connected.to_string()),
        kv("连接中", stats.connecting.to_string()),
        kv("错误", stats.error.to_string()),
        kv("已断开", stats.disconnected.to_string()),
        kv("Agent数量", app.agents.len().to_string()),
        kv("当前焦点", app.focus_area.label(app.locale).to_string()),
    ]
}

fn capability_lines(app: &TuiApp) -> Vec<Line<'static>> {
    vec![
        kv(
            "服务管理",
            "列表 / 添加 / 详情 / 连接 / 断开 / 重启 / 删除".to_string(),
        ),
        kv(
            "工具管理",
            "全局工具列表 / 按服务工具列表 / 工具详情 / 工具测试".to_string(),
        ),
        kv("Agent", "Agent列表 / 授权服务 / 解除授权".to_string()),
        kv(
            "日志",
            "运行消息 / Store事件 / 服务状态 / 日志配置".to_string(),
        ),
        kv(
            "状态",
            "运行概览 / 缓存健康 / 事件能力 / 功能清单".to_string(),
        ),
        kv("设置", "安装状态 / 常规配置 / 日志配置".to_string()),
        kv(
            "资源/Prompt",
            "Store API 已具备，TUI 暂以状态能力展示".to_string(),
        ),
        kv("后端迁移", "CLI/API 已具备，TUI 暂不绑定快捷键".to_string()),
        kv(
            "当前页面",
            i18n::text(app.locale, TextKey::NavStatus).to_string(),
        ),
    ]
}

fn kv(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), theme::muted()),
        Span::styled(value, theme::text()),
    ])
}
