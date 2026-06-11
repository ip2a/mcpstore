use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::{
    app::{FocusArea, TuiApp},
    i18n::{self, TextKey},
    theme, widgets,
};

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    widgets::chrome::render_control_bar(
        frame,
        area,
        app,
        Line::from(vec![
            Span::styled(
                if app.focus_area == FocusArea::ViewFilter {
                    "> "
                } else {
                    "  "
                },
                theme::accent(),
            ),
            Span::styled(
                i18n::text(app.locale, TextKey::FocusControlBar),
                theme::muted(),
            ),
            Span::raw("  "),
            Span::styled(
                i18n::text(app.locale, TextKey::ControlBarPlaceholder),
                theme::disabled(),
            ),
        ]),
    );
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &TuiApp, title_key: TextKey) {
    let title = i18n::text(app.locale, title_key);
    let content = Paragraph::new(Line::from(format!(
        "{} {}",
        title,
        i18n::text(app.locale, TextKey::PlaceholderBodySuffix)
    )))
    .block(widgets::chrome::panel_block(
        format!(
            "{} / {}",
            i18n::text(app.locale, TextKey::ContentRegion),
            title
        ),
        app.focus_area == FocusArea::ViewTable,
    ))
    .style(theme::muted());
    frame.render_widget(content, area);
}
