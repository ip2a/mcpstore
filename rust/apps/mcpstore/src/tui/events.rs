use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{
    AddServicePane, ContentPane, FocusArea, LogsPane, LogsSection, MainView, ServiceListMenu,
    ServiceManagementTab, SettingsPane, SettingsSection, StatusSection, TuiApp,
};
use super::i18n::{self, TextKey};
use crate::BoxErr;

pub fn handle_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return Ok(());
    }

    if app.select_modal.is_some() {
        app.handle_select_input(key);
        return Ok(());
    }

    if app.edit_modal.is_some() {
        app.handle_edit_input(key);
        return Ok(());
    }

    if app.show_tool_detail {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => app.close_tool_detail(),
            KeyCode::Char('t') => app.open_tool_test_editor(),
            _ => {}
        }
        return Ok(());
    }

    if app.show_service_detail {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => app.close_service_detail(),
            _ => {}
        }
        return Ok(());
    }

    if let Some(_pending) = app.pending_action.as_ref() {
        return handle_pending_action(app, rt, key);
    }

    if app.filter.search_mode {
        app.handle_search_input(key);
        return Ok(());
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
        handle_search_shortcut(app);
        return Ok(());
    }

    if key.code == KeyCode::Esc {
        handle_escape(app);
        return Ok(());
    }

    if app.active_view == MainView::Settings && app.focus_area == FocusArea::ViewTable {
        return handle_settings_content_key(app, key);
    }

    if app.active_view == MainView::Logs && app.focus_area == FocusArea::ViewTable {
        return handle_logs_content_key(app, key);
    }

    if app.active_view == MainView::ServiceManagement && app.focus_area == FocusArea::ViewTable {
        return handle_service_management_content_key(app, rt, key);
    }

    if app.active_view == MainView::Tools && app.focus_area == FocusArea::ViewTable {
        return handle_tools_content_key(app, rt, key);
    }

    if app.active_view == MainView::Agents && app.focus_area == FocusArea::ViewTable {
        return handle_agents_content_key(app, key);
    }

    if app.active_view == MainView::Status && app.focus_area == FocusArea::ViewTable {
        return handle_status_content_key(app, rt, key);
    }

    match key.code {
        KeyCode::Down => app.focus_next_area(),
        KeyCode::Up => app.focus_previous_area(),
        KeyCode::Tab => app.focus_next_area(),
        KeyCode::BackTab => app.focus_previous_area(),
        KeyCode::Char('r') => {
            if app.active_view == MainView::Tools {
                app.queue_tool_refresh();
            } else if app.active_view == MainView::Agents {
                app.queue_agent_refresh();
            } else if app.active_view == MainView::Status {
                app.refresh_status_sources(rt);
                app.status_message = format!(
                    "{} {}",
                    i18n::text(app.locale, TextKey::StatusSuccessPrefix),
                    i18n::text(app.locale, TextKey::RefreshedStatusInfo)
                );
            } else {
                app.refresh(rt, true)?;
                app.status_message = format!(
                    "{} {}",
                    i18n::text(app.locale, TextKey::StatusSuccessPrefix),
                    i18n::text(app.locale, TextKey::RefreshedServiceList)
                );
            }
        }
        _ => handle_focused_key(app, rt, key)?,
    }

    Ok(())
}

fn handle_focused_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match app.focus_area {
        FocusArea::MainNav => handle_nav_key(app, key),
        FocusArea::ViewFilter => handle_filter_key(app, rt, key),
        FocusArea::ViewTable => handle_table_key(app, rt, key),
    }
}

fn handle_nav_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Right | KeyCode::Char('l') => app.next_view(),
        KeyCode::Left | KeyCode::Char('h') => app.previous_view(),
        _ => {}
    }
    Ok(())
}

fn handle_filter_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    if app.active_view == MainView::ServiceManagement {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => app.previous_service_tab(),
            KeyCode::Right | KeyCode::Char('l') => app.next_service_tab(),
            KeyCode::Char('1') => app.select_service_tab(ServiceManagementTab::Services),
            KeyCode::Char('2') => app.select_service_tab(ServiceManagementTab::AddService),
            _ => {}
        }
        return Ok(());
    }

    if app.active_view == MainView::Tools {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => app.previous_tool_filter(rt),
            KeyCode::Right | KeyCode::Char('l') => app.next_tool_filter(rt),
            KeyCode::Char('r') => app.queue_tool_refresh(),
            _ => {}
        }
        return Ok(());
    }

    if app.active_view == MainView::Agents {
        match key.code {
            KeyCode::Char('r') => app.queue_agent_refresh(),
            KeyCode::Char('e') => app.open_agent_id_editor(),
            KeyCode::Char('a') => app.open_agent_assign_editor(),
            _ => {}
        }
        return Ok(());
    }

    if app.active_view != MainView::ServiceManagement {
        return Ok(());
    }

    Ok(())
}

fn handle_agents_content_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_agent_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_agent_services(),
        KeyCode::Up if app.agent_pane == ContentPane::Menu && app.selected_agent == 0 => {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => match app.agent_pane {
            ContentPane::Menu => app.previous_agent(),
            ContentPane::Body => app.previous_agent_service(),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.agent_pane {
            ContentPane::Menu => app.next_agent(),
            ContentPane::Body => app.next_agent_service(),
        },
        KeyCode::Enter => {
            if app.agent_pane == ContentPane::Menu {
                app.focus_agent_services();
            }
        }
        KeyCode::Char('r') => app.queue_agent_refresh(),
        KeyCode::Char('e') => app.open_agent_id_editor(),
        KeyCode::Char('a') => app.open_agent_assign_editor(),
        KeyCode::Char('u') if app.agent_pane == ContentPane::Body => app.queue_agent_unassign(),
        _ => {}
    }
    Ok(())
}

fn handle_status_content_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_status_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_status_body(),
        KeyCode::Up
            if app.status_pane == ContentPane::Menu
                && app.status_section == StatusSection::Overview =>
        {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.status_pane == ContentPane::Menu {
                app.previous_status_section();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.status_pane == ContentPane::Menu {
                app.next_status_section();
            }
        }
        KeyCode::Enter => {
            if app.status_pane == ContentPane::Menu {
                app.focus_status_body();
            }
        }
        KeyCode::Char('r') => {
            app.refresh_status_sources(rt);
            app.status_message = format!(
                "{} {}",
                i18n::text(app.locale, TextKey::StatusSuccessPrefix),
                i18n::text(app.locale, TextKey::RefreshedStatusInfo)
            );
        }
        _ => {}
    }
    Ok(())
}

fn handle_tools_content_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_tool_service_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_tool_list(),
        KeyCode::Up if app.tool_pane == ContentPane::Menu && app.selected_tool_service == 0 => {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => match app.tool_pane {
            ContentPane::Menu => app.previous_tool_service(rt),
            ContentPane::Body => app.previous_tool(),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.tool_pane {
            ContentPane::Menu => app.next_tool_service(rt),
            ContentPane::Body => app.next_tool(),
        },
        KeyCode::Enter => {
            if app.tool_pane == ContentPane::Menu {
                app.focus_tool_list();
            } else {
                app.open_selected_tool_detail();
            }
        }
        KeyCode::Char('r') => app.queue_tool_refresh(),
        KeyCode::Char('t') if app.tool_pane == ContentPane::Body => app.open_tool_test_editor(),
        _ => {}
    }
    Ok(())
}

fn handle_table_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    if app.active_view != MainView::ServiceManagement
        || app.service_tab != ServiceManagementTab::Services
        || app.service_list_pane != ContentPane::Body
    {
        return Ok(());
    }

    match key.code {
        KeyCode::Char('k') => app.move_selection(-1, rt)?,
        KeyCode::Char('j') => app.move_selection(1, rt)?,
        KeyCode::Char('g') => app.jump_to(0, rt)?,
        KeyCode::Char('G') => {
            if !app.filtered_services.is_empty() {
                app.jump_to(app.filtered_services.len() - 1, rt)?;
            }
        }
        KeyCode::Char('c') => app.connect_selected(rt)?,
        KeyCode::Char('d') => app.disconnect_selected(rt)?,
        KeyCode::Char('x') => app.restart_selected(rt)?,
        KeyCode::Char('D') => app.prompt_remove(),
        _ => {}
    }

    Ok(())
}

fn handle_service_management_content_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match app.service_tab {
        ServiceManagementTab::Services => handle_service_list_content_key(app, rt, key),
        ServiceManagementTab::AddService => handle_add_service_content_key(app, key),
    }
}

fn handle_settings_content_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_settings_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_settings_detail(),
        KeyCode::Up
            if app.settings_pane == SettingsPane::Menu
                && app.settings_section == SettingsSection::Status =>
        {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.settings_pane == SettingsPane::Menu {
                app.previous_settings_section();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.settings_pane == SettingsPane::Menu {
                app.next_settings_section();
            }
        }
        KeyCode::Enter => {
            if app.settings_pane == SettingsPane::Detail {
                app.open_settings_editor();
            } else {
                app.focus_settings_detail();
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_logs_content_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_logs_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_logs_body(),
        KeyCode::Up
            if app.logs_pane == LogsPane::Menu && app.logs_section == LogsSection::Runtime =>
        {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.logs_pane == LogsPane::Menu {
                app.previous_logs_section();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.logs_pane == LogsPane::Menu {
                app.next_logs_section();
            }
        }
        KeyCode::Enter => {
            if app.logs_pane == LogsPane::Menu {
                app.focus_logs_body();
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_service_list_content_key(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_service_list_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_service_list_body(),
        KeyCode::Up
            if app.service_list_pane == ContentPane::Menu
                && app.service_list_menu == ServiceListMenu::All =>
        {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => match app.service_list_pane {
            ContentPane::Menu => app.previous_service_list_menu_item(rt),
            ContentPane::Body => app.move_selection(-1, rt)?,
        },
        KeyCode::Down | KeyCode::Char('j') => match app.service_list_pane {
            ContentPane::Menu => app.next_service_list_menu_item(rt),
            ContentPane::Body => app.move_selection(1, rt)?,
        },
        KeyCode::Enter => {
            if app.service_list_pane == ContentPane::Menu {
                app.focus_service_list_body();
            } else {
                app.open_selected_detail(rt)?;
            }
        }
        KeyCode::Char('c') if app.service_list_pane == ContentPane::Body => {
            app.connect_selected(rt)?
        }
        KeyCode::Char('d') if app.service_list_pane == ContentPane::Body => {
            app.disconnect_selected(rt)?
        }
        KeyCode::Char('x') if app.service_list_pane == ContentPane::Body => {
            app.restart_selected(rt)?
        }
        KeyCode::Char('D') if app.service_list_pane == ContentPane::Body => app.prompt_remove(),
        _ => {}
    }
    Ok(())
}

fn handle_search_shortcut(app: &mut TuiApp) {
    if app.active_view == MainView::ServiceManagement
        && app.service_tab == ServiceManagementTab::Services
    {
        app.filter.search_mode = true;
        app.status_message = format!(
            "{} {}",
            i18n::text(app.locale, TextKey::StatusInProgressPrefix),
            i18n::text(app.locale, TextKey::SearchMode)
        );
    } else {
        app.status_message = format!(
            "{} {}",
            i18n::text(app.locale, TextKey::StatusInProgressPrefix),
            i18n::text(app.locale, TextKey::PageNotSupportSearch)
        );
    }
}

fn handle_escape(app: &mut TuiApp) {
    match app.focus_area {
        FocusArea::MainNav => app.should_quit = true,
        FocusArea::ViewFilter => app.focus_previous_area(),
        FocusArea::ViewTable => match app.active_view {
            MainView::ServiceManagement => match app.service_tab {
                ServiceManagementTab::Services => {
                    if app.service_list_pane == ContentPane::Body {
                        app.focus_service_list_menu();
                    } else {
                        app.focus_previous_area();
                    }
                }
                ServiceManagementTab::AddService => {
                    if app.add_service.pane == AddServicePane::Form {
                        app.focus_add_service_menu();
                    } else {
                        app.focus_previous_area();
                    }
                }
            },
            MainView::Settings => {
                if app.settings_pane == SettingsPane::Detail {
                    app.focus_settings_menu();
                } else {
                    app.focus_previous_area();
                }
            }
            MainView::Logs => {
                if app.logs_pane == LogsPane::Body {
                    app.focus_logs_menu();
                } else {
                    app.focus_previous_area();
                }
            }
            MainView::Tools => {
                if app.tool_pane == ContentPane::Body {
                    app.focus_tool_service_menu();
                } else {
                    app.focus_previous_area();
                }
            }
            MainView::Agents => {
                if app.agent_pane == ContentPane::Body {
                    app.focus_agent_menu();
                } else {
                    app.focus_previous_area();
                }
            }
            MainView::Status => {
                if app.status_pane == ContentPane::Body {
                    app.focus_status_menu();
                } else {
                    app.focus_previous_area();
                }
            }
        },
    }
}

fn handle_add_service_content_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.focus_add_service_menu(),
        KeyCode::Right | KeyCode::Char('l') => app.focus_add_service_form(),
        KeyCode::Up
            if app.add_service.pane == AddServicePane::Menu
                && app.add_service.selected_section == 0 =>
        {
            app.focus_previous_area()
        }
        KeyCode::Up | KeyCode::Char('k') => match app.add_service.pane {
            AddServicePane::Menu => app.previous_add_service_menu_item(),
            AddServicePane::Form => app.previous_add_service_form_field(),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.add_service.pane {
            AddServicePane::Menu => app.next_add_service_menu_item(),
            AddServicePane::Form => app.next_add_service_form_field(),
        },
        KeyCode::Enter => {
            if app.add_service.pane == AddServicePane::Form {
                app.open_add_service_editor();
            } else {
                app.focus_add_service_form();
            }
        }
        KeyCode::Char('a') => app.submit_add_service(),
        _ => {}
    }
    Ok(())
}

fn handle_pending_action(
    app: &mut TuiApp,
    rt: &tokio::runtime::Runtime,
    key: KeyEvent,
) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Char('y') => app.confirm_remove(rt)?,
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_pending(),
        _ => {}
    }
    Ok(())
}
