use crate::tui::{
    app::{MainView, TuiApp},
    i18n::{Locale, TextKey},
};
use ratatui::{layout::Rect, Frame};

pub mod add_service;
pub mod agents;
pub mod logs;
pub mod placeholder;
pub mod service_management;
pub mod services;
pub mod settings;
pub mod status;
pub mod tools;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageDescriptor {
    pub id: MainView,
    pub title_key: TextKey,
    pub enabled: bool,
    pub order: u16,
}

impl PageDescriptor {
    pub fn title(&self, locale: Locale) -> &'static str {
        crate::tui::i18n::text(locale, self.title_key)
    }
}

const REGISTERED_PAGES: [PageDescriptor; 6] = [
    PageDescriptor {
        id: MainView::ServiceManagement,
        title_key: TextKey::NavServiceManagement,
        enabled: true,
        order: 10,
    },
    PageDescriptor {
        id: MainView::Tools,
        title_key: TextKey::NavTools,
        enabled: true,
        order: 20,
    },
    PageDescriptor {
        id: MainView::Agents,
        title_key: TextKey::NavAgents,
        enabled: true,
        order: 30,
    },
    PageDescriptor {
        id: MainView::Logs,
        title_key: TextKey::NavLogs,
        enabled: true,
        order: 40,
    },
    PageDescriptor {
        id: MainView::Status,
        title_key: TextKey::NavStatus,
        enabled: true,
        order: 50,
    },
    PageDescriptor {
        id: MainView::Settings,
        title_key: TextKey::NavSettings,
        enabled: true,
        order: 60,
    },
];

pub fn registered_pages() -> &'static [PageDescriptor] {
    &REGISTERED_PAGES
}

pub fn visible_pages() -> Vec<PageDescriptor> {
    let mut pages: Vec<PageDescriptor> = registered_pages()
        .iter()
        .copied()
        .filter(|page| page.enabled)
        .collect();
    pages.sort_by_key(|page| page.order);
    pages
}

pub fn descriptor_for(id: MainView) -> Option<PageDescriptor> {
    registered_pages()
        .iter()
        .copied()
        .find(|page| page.id == id)
}

pub fn render_control_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    match app.active_view {
        MainView::ServiceManagement => service_management::render_control_bar(frame, area, app),
        MainView::Tools => tools::render_control_bar(frame, area, app),
        MainView::Agents => agents::render_control_bar(frame, area, app),
        MainView::Logs => logs::render_control_bar(frame, area, app),
        MainView::Status => status::render_control_bar(frame, area, app),
        MainView::Settings => settings::render_control_bar(frame, area, app),
    }
}

pub fn render_content(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    match app.active_view {
        MainView::ServiceManagement => service_management::render_content(frame, area, app),
        MainView::Tools => tools::render_content(frame, area, app),
        MainView::Agents => agents::render_content(frame, area, app),
        MainView::Logs => logs::render_content(frame, area, app),
        MainView::Status => status::render_content(frame, area, app),
        MainView::Settings => settings::render_content(frame, area, app),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_pages_are_enabled_and_ordered() {
        let pages = visible_pages();
        assert_eq!(
            pages.first().map(|page| page.id),
            Some(MainView::ServiceManagement)
        );
        assert!(pages.iter().all(|page| page.enabled));
        assert!(pages.windows(2).all(|pair| pair[0].order <= pair[1].order));
    }
}
