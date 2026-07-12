use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{AddServiceField, AddServiceMode, AddServicePane, FocusArea, TuiApp},
    i18n::{self, Locale, TextKey},
    layout as tui_layout, theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let mut spans = Vec::new();

    for (index, mode) in AddServiceMode::ALL.iter().enumerate() {
        let style = if *mode == app.add_service.mode {
            theme::tab_selected()
        } else {
            theme::text()
        };
        spans.push(Span::styled(
            format!(" {} {} ", index + 1, mode.label()),
            style,
        ));
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(
        i18n::text(app.locale, TextKey::AddServiceControlBarHint),
        theme::text(),
    ));

    widgets::chrome::render_control_bar(frame, area, app, Line::from(spans));
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tui_layout::CONTENT_MENU_PERCENT),
            Constraint::Percentage(tui_layout::CONTENT_BODY_PERCENT),
        ])
        .split(area);

    render_menu(frame, layout[0], app);
    render_form(frame, layout[1], app);
}

fn render_menu(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused =
        app.focus_area == FocusArea::ViewTable && app.add_service.pane == AddServicePane::Menu;
    let items = AddServiceMode::MENU
        .iter()
        .enumerate()
        .map(|(_index, mode)| {
            let selected = *mode == app.add_service.mode;
            let style = if selected {
                theme::menu_selected()
            } else {
                theme::text()
            };
            ListItem::new(Line::from(vec![
                Span::styled(if selected { "> " } else { "  " }, theme::accent()),
                Span::styled(mode.menu_label(), style),
            ]))
            .style(if selected {
                theme::menu_selected()
            } else {
                theme::text()
            })
        });

    let menu = List::new(items)
        .block(
            widgets::chrome::panel_block(i18n::text(app.locale, TextKey::AddMethod), focused)
                .padding(Padding::horizontal(1)),
        )
        .style(theme::text());
    frame.render_widget(menu, area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_summary(frame, layout[0], app);
    render_fields(frame, layout[1], app);
    render_hint(frame, layout[2], app);
}

fn render_summary(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused =
        app.focus_area == FocusArea::ViewTable && app.add_service.pane == AddServicePane::Form;
    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(i18n::text(app.locale, TextKey::ModeLabel), theme::muted()),
            Span::styled(app.add_service.mode.label(), theme::field_label()),
            Span::raw("    "),
            Span::styled(i18n::text(app.locale, TextKey::ModuleLabel), theme::muted()),
            Span::styled(app.add_service.mode.menu_label(), theme::field_label()),
            Span::raw("    "),
            Span::styled(
                format!(
                    "{}{}",
                    i18n::text(app.locale, TextKey::FocusLabel),
                    if focused {
                        i18n::text(app.locale, TextKey::FocusRightForm)
                    } else {
                        i18n::text(app.locale, TextKey::FocusLeftMenu)
                    }
                ),
                theme::text(),
            ),
        ]),
        Line::from(vec![
            Span::styled(i18n::text(app.locale, TextKey::NameLabel), theme::muted()),
            Span::styled(empty_label(&app.add_service.name), theme::field_value()),
            Span::raw("    "),
            Span::styled(i18n::text(app.locale, TextKey::ScopeLabel), theme::muted()),
            Span::styled(&app.add_service.scope, theme::field_value()),
            Span::raw("    "),
            Span::styled(
                i18n::text(app.locale, TextKey::ConnectLabel),
                theme::muted(),
            ),
            Span::styled(&app.add_service.connect_after_add, theme::field_value()),
        ]),
    ])
    .block(
        widgets::chrome::panel_block(i18n::text(app.locale, TextKey::Summary), false)
            .padding(Padding::horizontal(1)),
    )
    .style(theme::text());
    frame.render_widget(summary, area);
}

fn render_fields(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let focused =
        app.focus_area == FocusArea::ViewTable && app.add_service.pane == AddServicePane::Form;
    let fields = app.add_service.selected_fields();
    let items = fields.iter().enumerate().map(|(index, field)| {
        let selected = focused && index == app.add_service.selected_field;
        let line = if *field == AddServiceField::Submit {
            Line::from(vec![
                Span::styled(if selected { "> " } else { "  " }, theme::accent()),
                Span::styled(
                    i18n::text(app.locale, TextKey::AddService),
                    theme::field_label(),
                ),
                Span::styled(
                    i18n::text(app.locale, TextKey::AddServiceSubmitHint),
                    theme::text(),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(if selected { "> " } else { "  " }, theme::accent()),
                Span::styled(format!("{:<18}", field.label()), theme::field_label()),
                Span::styled(compact_value(app, *field, area.width), theme::field_value()),
            ])
        };

        ListItem::new(line).style(if selected {
            theme::field_selected()
        } else {
            theme::text()
        })
    });

    let list = List::new(items)
        .block(
            widgets::chrome::panel_block(
                format!(
                    "{} / {} / {}",
                    i18n::text(app.locale, TextKey::ContentRegion),
                    app.add_service.mode.label(),
                    app.add_service.mode.menu_label()
                ),
                focused,
            )
            .padding(Padding::horizontal(1)),
        )
        .style(theme::text());
    frame.render_widget(list, area);
}

fn render_hint(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let hint = selected_hint(app);
    let paragraph = Paragraph::new(vec![
        Line::from(Span::styled(hint, theme::text())),
        Line::from(Span::styled(
            i18n::text(app.locale, TextKey::AddServiceControlHint),
            theme::muted(),
        )),
    ])
    .block(
        widgets::chrome::panel_block(i18n::text(app.locale, TextKey::Actions), false)
            .padding(Padding::horizontal(1)),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn selected_hint(app: &TuiApp) -> &'static str {
    let fields = app.add_service.selected_fields();
    if fields == [AddServiceField::Submit] {
        return i18n::text(app.locale, TextKey::SubmitHint);
    }
    field_help(
        app.add_service.mode,
        app.add_service.selected_field(),
        app.locale,
    )
}

fn field_value(app: &TuiApp, field: AddServiceField) -> String {
    match field {
        AddServiceField::Name => empty_label(&app.add_service.name),
        AddServiceField::Command => empty_label(&app.add_service.command),
        AddServiceField::Args => empty_label(&app.add_service.args),
        AddServiceField::Url => empty_label(&app.add_service.url),
        AddServiceField::Description => empty_label(&app.add_service.description),
        AddServiceField::WorkingDir => empty_label(&app.add_service.working_dir),
        AddServiceField::Env => empty_label(&app.add_service.env),
        AddServiceField::Headers => empty_label(&app.add_service.headers),
        AddServiceField::Scope => empty_label(&app.add_service.scope),
        AddServiceField::Agent => empty_label(&app.add_service.agent),
        AddServiceField::ConnectAfterAdd => empty_label(&app.add_service.connect_after_add),
        AddServiceField::Json => empty_label(&app.add_service.json),
        AddServiceField::Toml => empty_label(&app.add_service.toml),
        AddServiceField::Submit => "submit".to_string(),
    }
}

fn field_help(mode: AddServiceMode, field: AddServiceField, locale: Locale) -> &'static str {
    match field {
        AddServiceField::Name => i18n::text(locale, TextKey::FieldHelpName),
        AddServiceField::Command => i18n::text(locale, TextKey::FieldHelpCommand),
        AddServiceField::Args => i18n::text(locale, TextKey::FieldHelpArgs),
        AddServiceField::Url => i18n::text(locale, TextKey::FieldHelpUrl),
        AddServiceField::Description => i18n::text(locale, TextKey::FieldHelpDescription),
        AddServiceField::WorkingDir => i18n::text(locale, TextKey::FieldHelpWorkingDir),
        AddServiceField::Env => i18n::text(locale, TextKey::FieldHelpEnv),
        AddServiceField::Headers => i18n::text(locale, TextKey::FieldHelpHeaders),
        AddServiceField::Scope => i18n::text(locale, TextKey::FieldHelpScope),
        AddServiceField::Agent => i18n::text(locale, TextKey::FieldHelpAgent),
        AddServiceField::ConnectAfterAdd => i18n::text(locale, TextKey::FieldHelpConnectAfterAdd),
        AddServiceField::Json if mode == AddServiceMode::Json => {
            i18n::text(locale, TextKey::FieldHelpJson)
        }
        AddServiceField::Toml if mode == AddServiceMode::Toml => {
            i18n::text(locale, TextKey::FieldHelpToml)
        }
        _ => i18n::text(locale, TextKey::FieldHelpDefault),
    }
}

fn empty_label(value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        value.to_string()
    }
}

fn compact_value(app: &TuiApp, field: AddServiceField, width: u16) -> String {
    let value = field_value(app, field).replace('\n', " / ");
    let max = width.saturating_sub(28).max(12) as usize;
    truncate(&value, max)
}

fn truncate(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}
