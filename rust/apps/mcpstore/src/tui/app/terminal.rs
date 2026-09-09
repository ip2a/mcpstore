use super::*;
use crate::{
    bootstrap,
    store_args::{open_store_access, StoreSourceArgs},
};
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
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
}

pub fn run(
    args: &StoreSourceArgs,
    embedded: bool,
    tick_ms: u64,
    locale_override: Option<Locale>,
) -> Result<(), BoxErr> {
    let rt = bootstrap::build_runtime()?;
    let mut access = rt.block_on(open_store_access(args, embedded, None))?;

    // 引导信息一次取齐（embedded 也走同一 op 分发）
    let info = rt
        .block_on(access.request(
            crate::daemon::protocol::KernelOperation::GetAppConfig,
            serde_json::json!({}),
        ))
        .map_err(|error| format!("Failed to read runtime configuration: {error}"))?;
    let app_config: mcpstore::config::AppConfig = serde_json::from_value(info["config"].clone())?;
    let config_path = info["mcp_path"].as_str().unwrap_or_default().to_string();
    bootstrap::init_tracing_from_config(Some(&app_config));

    let locale = locale_override
        .or_else(|| Locale::from_config_value(&app_config.ui.language))
        .unwrap_or_default();
    let cache_storage_label = info["current_store_name"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let namespace = info["namespace"].as_str().unwrap_or("?").to_string();
    let mcp_aggregate_transport = app_config.mcp_aggregate.transport.clone();
    let mcp_aggregate_port = app_config.mcp_aggregate.port;

    let mut app = TuiApp::new(
        access,
        app_config,
        Duration::from_millis(tick_ms),
        locale,
        args.source.as_str().to_string(),
        cache_storage_label,
        namespace,
        config_path,
        mcp_aggregate_transport,
        mcp_aggregate_port,
    );
    app.refresh(&rt, false)?;
    app.status_message = i18n::text(app.locale, TextKey::TuiReady).to_string();

    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        app.sync_status_history();
        terminal.draw(|frame| super::super::ui::draw(frame, &mut app))?;
        if app.has_pending_task() {
            if let Err(error) = app.process_pending_task(&rt) {
                app.status_message = format!("[Error] {error}");
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
                        app.status_message = format!("[Error] {error}");
                    }
                }
            }
        }
    }

    terminal.show_cursor()?;
    rt.block_on(app.access.close());
    Ok(())
}
