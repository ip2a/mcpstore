use ratatui::layout::Constraint;

pub const HEADER_BORDER_HEIGHT: u16 = 1;
pub const HEADER_BANNER_PERCENT: u16 = 56;
pub const HEADER_STATUS_PERCENT: u16 = 44;
pub const BANNER_HORIZONTAL_MARGIN: u16 = 1;
pub const STATUS_HORIZONTAL_MARGIN: u16 = 1;

pub const MAIN_NAV_HEIGHT: u16 = 2;
pub const MAIN_CONTENT_MIN_HEIGHT: u16 = 10;

pub const CONTROL_BAR_HEIGHT: u16 = 1;
pub const CONTROL_CONTENT_GAP_HEIGHT: u16 = 1;
pub const VIEW_CONTENT_MIN_HEIGHT: u16 = 8;
pub const CONTENT_MENU_PERCENT: u16 = 18;
pub const CONTENT_BODY_PERCENT: u16 = 82;
pub const SETTINGS_MENU_PERCENT: u16 = 25;
pub const SETTINGS_DETAIL_PERCENT: u16 = 75;

pub const FILTER_STATUS_MIN_WIDTH: u16 = 40;
pub const FILTER_SEARCH_MIN_WIDTH: u16 = 30;

pub const CONFIRM_DIALOG_PERCENT_X: u16 = 60;
pub const CONFIRM_DIALOG_PERCENT_Y: u16 = 24;
pub const INPUT_DIALOG_PERCENT_X: u16 = 72;
pub const INPUT_DIALOG_PERCENT_Y: u16 = 34;
pub const LOADING_DIALOG_PERCENT_X: u16 = 50;
pub const LOADING_DIALOG_PERCENT_Y: u16 = 18;

pub fn service_table_widths() -> [Constraint; 6] {
    [
        Constraint::Length(24),
        Constraint::Length(12),
        Constraint::Length(17),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Min(14),
    ]
}
