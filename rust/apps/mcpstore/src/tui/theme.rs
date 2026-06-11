use ratatui::style::{Color, Modifier, Style};

pub fn text() -> Style {
    Style::default().fg(Color::Black)
}

pub fn modal_overlay() -> Style {
    Style::default().fg(Color::Black).bg(Color::White)
}

pub fn modal_surface() -> Style {
    Style::default().fg(Color::Black).bg(Color::White)
}

pub fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn disabled() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn accent_bold() -> Style {
    accent().add_modifier(Modifier::BOLD)
}

pub fn selected_label() -> Style {
    accent().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

pub fn tab_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Rgb(178, 240, 248))
        .add_modifier(Modifier::BOLD)
}

pub fn menu_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Rgb(216, 246, 250))
        .add_modifier(Modifier::BOLD)
}

pub fn field_label() -> Style {
    Style::default()
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

pub fn field_value() -> Style {
    Style::default().fg(Color::Black)
}

pub fn field_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Rgb(226, 248, 242))
        .add_modifier(Modifier::BOLD)
}

pub fn panel_title() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn table_header() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn table_row_highlight() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Rgb(210, 244, 246))
        .add_modifier(Modifier::BOLD)
}

pub fn danger() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub fn success() -> Style {
    Style::default().fg(Color::Green)
}

pub fn warning() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn error() -> Style {
    Style::default().fg(Color::Red)
}
