use crossterm::event::{KeyCode, KeyEvent};

use super::app::TuiApp;
use super::widgets::filter_bar::FilterStatus;
use crate::BoxErr;

pub fn handle_key(app: &mut TuiApp, rt: &tokio::runtime::Runtime, key: KeyEvent) -> Result<(), BoxErr> {
    if let Some(_pending) = app.pending_action.as_ref() {
        return handle_pending_action(app, rt, key);
    }

    if app.filter.search_mode {
        app.handle_search_input(key);
        return Ok(());
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1, rt)?,
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1, rt)?,
        KeyCode::Char('g') => app.jump_to(0, rt)?,
        KeyCode::Char('G') => {
            if !app.filtered_services.is_empty() {
                app.jump_to(app.filtered_services.len() - 1, rt)?;
            }
        }
        KeyCode::Char('r') => {
            app.refresh(rt, true)?;
            app.status_message = "[成功] 已刷新服务列表".to_string();
        }
        KeyCode::Char('c') => app.connect_selected(rt)?,
        KeyCode::Char('d') => app.disconnect_selected(rt)?,
        KeyCode::Char('x') => app.restart_selected(rt)?,
        KeyCode::Char('D') => app.prompt_remove(),
        KeyCode::Char('1') => app.set_status_filter(FilterStatus::All, rt),
        KeyCode::Char('2') => app.set_status_filter(FilterStatus::Connected, rt),
        KeyCode::Char('3') => app.set_status_filter(FilterStatus::Error, rt),
        KeyCode::Char('4') => app.set_status_filter(FilterStatus::Disconnected, rt),
        KeyCode::Char('5') => app.set_status_filter(FilterStatus::Connecting, rt),
        KeyCode::Char('s') => {
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                app.toggle_sort_direction();
            } else {
                app.toggle_sort();
            }
            app.status_message = format!(
                "[成功] 排序: {} {}",
                app.filter.sort_by.label(),
                if app.filter.sort_asc { "升序" } else { "降序" }
            );
        }
        KeyCode::Char('/') => {
            app.filter.search_mode = true;
            app.status_message = "[进行中] 搜索模式: 输入关键词，Esc 退出".to_string();
        }
        _ => {}
    }

    Ok(())
}

fn handle_pending_action(app: &mut TuiApp, rt: &tokio::runtime::Runtime, key: KeyEvent) -> Result<(), BoxErr> {
    match key.code {
        KeyCode::Char('y') => app.confirm_remove(rt)?,
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_pending(),
        _ => {}
    }
    Ok(())
}
