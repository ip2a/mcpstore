use super::*;
use crate::{bootstrap, store_args::StoreSourceArgs};
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

struct TerminalGuard;

impl TerminalGuard {
    pub fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen).ok();
    }
}

pub fn run(
    args: &StoreSourceArgs,
    tick_ms: u64,
    locale_override: Option<Locale>,
) -> Result<(), BoxErr> {
    bootstrap::init_tracing_silent("mcpstore=warn");

    let rt = bootstrap::build_runtime()?;
    let store = crate::store_args::build_store(args)?;
    rt.block_on(async { store.load_from_source().await })?;

    let app_config = store.config_manager().load_app_config_or_default()?;
    let locale = locale_override
        .or_else(|| Locale::from_config_value(&app_config.ui.language))
        .unwrap_or_default();
    let cache_storage_label =
        rt.block_on(async { store.current_cache_storage().await.as_str().to_string() });
    let namespace = store.namespace();
    let config_path = store.config_manager().mcp_path().display().to_string();

    let mut app = TuiApp::new(
        store,
        Duration::from_millis(tick_ms),
        locale,
        args.source.as_str().to_string(),
        cache_storage_label,
        namespace,
        config_path,
    );
    app.refresh(&rt, false)?;
    app.status_message = i18n::text(app.locale, TextKey::TuiReady).to_string();

    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| super::super::ui::draw(frame, &mut app, &rt))?;
        if app.has_pending_task() {
            if let Err(error) = app.process_pending_task(&rt) {
                app.status_message = format!("[错误] {error}");
            }
            continue;
        }

        if app.should_quit {
            break;
        }

        if event::poll(app.tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    if let Err(error) = super::super::events::handle_key(&mut app, &rt, key) {
                        app.status_message = format!("[错误] {error}");
                    }
                }
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}
