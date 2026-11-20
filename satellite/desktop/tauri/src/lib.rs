use anyhow::{anyhow, Result};
use reqwest::Client;
use std::collections::HashSet;
use std::ffi::OsString;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::{
    AppHandle, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

// Mirrors `web/src/App.tsx` shell: max-w-[1200px] + px-3 (12*2).
const WEB_CONTENT_MAX_WIDTH: f64 = 1200.0;
const WEB_CONTENT_HORIZONTAL_PADDING: f64 = 24.0;

/// Minimum width as a fraction of the 1200px content boundary (below the boundary is OK).
/// Effective min window width ≈ 1200 * ratio + 24. Tune while checking list/header layout.
const MIN_CONTENT_WIDTH_RATIO: f64 = 0.70;

/// Hard floor so ultra-narrow monitors still stay usable.
const ABSOLUTE_MIN_INNER_WIDTH: f64 = 720.0;

/// Height can shrink more aggressively than width; list pages scroll internally.
const DESIGN_MIN_INNER_HEIGHT: f64 = 480.0;

/// Default size targets Services 3×3 card grid with equal L/R insets:
/// sidebar 192 + shell 24 + list px-4 32 + 3×256 + 2×16 gap = 1048;
/// height ≈ chrome + 3×152 card rows (no 4th row peek).
const DEFAULT_INNER_WIDTH: f64 = 1048.0;
const DEFAULT_INNER_HEIGHT: f64 = 720.0;

/// Loopback hosts that must bypass HTTP(S)_PROXY. The desktop shell and its
/// sidecar talk to the local panel/daemon over 127.0.0.1; a user-wide proxy
/// (Clash/Surge/etc.) that also captures loopback makes health checks and the
/// WebView look "blank" even though the panel is fine. External hosts (GitHub,
/// plugin marketplaces, remote MCP) stay on the user's proxy.
const LOOPBACK_NO_PROXY_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

#[derive(Clone, Copy)]
struct MinInnerSize {
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
struct WindowState {
    width: u32,
    height: u32,
}

/// The desktop loads the standalone web panel process (`mcpstore web`), which
/// owns the /api face and the static frontend. The daemon is only attached or
/// started first because the panel forwards every resource request to it.
fn panel_url(app: &AppHandle) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        if !mcpstore_cli::daemon::ensure::is_daemon_ready().await {
            start_daemon(app)?;
            mcpstore_cli::daemon::ensure::wait_daemon_ready(Duration::from_secs(30)).await?;
        }

        // Same precedence as the panel itself: config.toml [web], default port.
        let web = mcpstore_cli::ConfigManager::new()
            .load_app_config_or_default()
            .map_err(|error| anyhow!("failed to load config.toml: {error}"))?
            .web;
        let url = format!("http://127.0.0.1:{}", web.port);
        // Health checks are loopback-only; never send them through a user proxy.
        let client = Client::builder()
            .no_proxy()
            .build()
            .map_err(|error| anyhow!("failed to build panel probe client: {error}"))?;
        if client
            .get(format!("{url}/api/health"))
            .send()
            .await
            .is_ok()
        {
            return Ok(url);
        }
        start_panel(app)?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if client
                .get(format!("{url}/api/health"))
                .send()
                .await
                .is_ok()
            {
                return Ok(url);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!("web panel did not become ready"));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
}

fn sidecar_binary() -> Result<std::path::PathBuf> {
    // tauri-build copies the externalBin sidecar (built as
    // binaries/mcpstore-<target-triple>) next to the compiled app binary with
    // the triple stripped: target/debug/mcpstore in dev, Contents/MacOS/mcpstore
    // in a packaged .app. It does NOT live under resource_dir().
    std::env::current_exe()?
        .parent()
        .map(|dir| {
            dir.join(if cfg!(windows) {
                "mcpstore.exe"
            } else {
                "mcpstore"
            })
        })
        .filter(|path| path.is_file())
        .ok_or_else(|| anyhow!("packaged mcpstore daemon binary is missing"))
}

fn spawn_sidecar(
    app: &AppHandle,
    arg: &'static str,
    log_name: &str,
    current_dir: Option<std::path::PathBuf>,
) -> Result<Child> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("home directory is unavailable"))?;
    let log_path = home.join(".mcpstore").join("logs").join(log_name);
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let mut command = Command::new(sidecar_binary()?);
    command.arg(arg);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    // Re-assert on the child even though the parent already primed env: some
    // launch paths (Finder vs terminal) inherit different proxy settings.
    apply_loopback_no_proxy_to_command(&mut command);
    Ok(command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?)
}

fn start_daemon(app: &AppHandle) -> Result<Child> {
    // Bundled plugins ship as Resources/plugins; the daemon's own first-launch
    // materialization scans cwd/plugins, so run it from resource_dir().
    let resource_dir = app.path().resource_dir()?;
    spawn_sidecar(app, "start", "desktop-daemon.out", Some(resource_dir))
}

fn start_panel(app: &AppHandle) -> Result<Child> {
    spawn_sidecar(app, "web", "desktop-panel.out", None)
}

fn compute_min_inner_size(window: &WebviewWindow) -> MinInnerSize {
    let boundary_width =
        WEB_CONTENT_MAX_WIDTH * MIN_CONTENT_WIDTH_RATIO + WEB_CONTENT_HORIZONTAL_PADDING;
    let mut min_width = boundary_width.max(ABSOLUTE_MIN_INNER_WIDTH);

    if let Ok(Some(monitor)) = window.current_monitor() {
        let scale_factor = monitor.scale_factor();
        if scale_factor > 0.0 {
            let area_width = (monitor.work_area().size.width as f64) / scale_factor;
            // Do not require a min wider than the usable desktop area.
            min_width = min_width.min(area_width * 0.94);
        }
    }

    MinInnerSize {
        width: min_width,
        height: DESIGN_MIN_INNER_HEIGHT,
    }
}

fn apply_min_inner_size(window: &WebviewWindow) {
    let min_size = compute_min_inner_size(window);
    let _ = window.set_min_size(Some(LogicalSize::new(min_size.width, min_size.height)));
}

fn prime_desktop_environment() {
    ensure_loopback_no_proxy();

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(shell_path) = read_login_shell_path() {
            let current = std::env::var_os("PATH");
            if let Some(merged) = merge_path_values(&shell_path, current) {
                std::env::set_var("PATH", merged);
            }
        }
    }
}

/// Merge loopback bypass hosts into NO_PROXY / no_proxy without wiping the
/// caller's existing exceptions (corp domains, etc.).
fn merge_no_proxy_value(existing: Option<&str>) -> String {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for host in LOOPBACK_NO_PROXY_HOSTS
        .iter()
        .copied()
        .chain(
            existing
                .into_iter()
                .flat_map(|value| value.split(','))
                .map(str::trim)
                .filter(|part| !part.is_empty()),
        )
    {
        let key = host.to_ascii_lowercase();
        if seen.insert(key) {
            merged.push(host.to_string());
        }
    }

    merged.join(",")
}

fn ensure_loopback_no_proxy() {
    for key in ["NO_PROXY", "no_proxy"] {
        let current = std::env::var(key).ok();
        let merged = merge_no_proxy_value(current.as_deref());
        std::env::set_var(key, merged);
    }
}

fn apply_loopback_no_proxy_to_command(command: &mut Command) {
    for key in ["NO_PROXY", "no_proxy"] {
        let current = std::env::var(key).ok();
        command.env(key, merge_no_proxy_value(current.as_deref()));
    }
}

#[cfg(not(target_os = "windows"))]
fn read_login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());
    let mut candidates = vec![
        shell,
        "/bin/zsh".to_string(),
        "/bin/bash".to_string(),
        "/bin/sh".to_string(),
    ];
    candidates.dedup();

    for program in candidates {
        let Ok(output) = Command::new(&program)
            .args(["-lc", "printf %s \"$PATH\""])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }

    None
}

fn merge_path_values(preferred: &str, current: Option<OsString>) -> Option<OsString> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for value in [Some(OsString::from(preferred)), current]
        .into_iter()
        .flatten()
    {
        for path in std::env::split_paths(&value) {
            if path.as_os_str().is_empty() {
                continue;
            }
            if seen.insert(path.clone()) {
                merged.push(path);
            }
        }
    }

    std::env::join_paths(merged).ok()
}

/// Resize the main window to the design default and persist via window-state.
#[tauri::command]
fn reset_default_window_size(app: AppHandle) -> std::result::Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;

    if window.is_fullscreen().map_err(|error| error.to_string())? {
        window
            .set_fullscreen(false)
            .map_err(|error| error.to_string())?;
    }
    if window.is_maximized().map_err(|error| error.to_string())? {
        window.unmaximize().map_err(|error| error.to_string())?;
    }

    window
        .set_size(LogicalSize::new(DEFAULT_INNER_WIDTH, DEFAULT_INNER_HEIGHT))
        .map_err(|error| error.to_string())?;
    window.center().map_err(|error| error.to_string())?;
    apply_min_inner_size(&window);

    app.save_window_state(StateFlags::all())
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Persists size/position/maximized/fullscreen per window label and
        // restores on next launch — the cross-platform standard from
        // docs/调研文档-成熟保存窗口大小方案v1.md. Registered before setup so the
        // state applies to the window created below.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![reset_default_window_size])
        .setup(|app| {
            prime_desktop_environment();
            if let Some(home) = dirs::home_dir() {
                let _ = std::env::set_current_dir(home);
            }

            let url = panel_url(&app.handle())?;
            let initial_state = WindowState {
                width: DEFAULT_INNER_WIDTH as u32,
                height: DEFAULT_INNER_HEIGHT as u32,
            };
            let builder = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("desktop URL is valid")),
            )
            .title("mcpstore");
            #[cfg(target_os = "macos")]
            let builder = builder.hidden_title(true);
            let window = builder
                .inner_size(initial_state.width as f64, initial_state.height as f64)
                .min_inner_size(ABSOLUTE_MIN_INNER_WIDTH, DESIGN_MIN_INNER_HEIGHT)
                .zoom_hotkeys_enabled(false)
                .center()
                .build()?;

            apply_min_inner_size(&window);

            let event_window = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Moved(_) => {
                    apply_min_inner_size(&event_window);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    apply_min_inner_size(&event_window);
                }
                _ => {}
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mcpstore tauri app");
}

#[cfg(test)]
mod tests {
    use super::merge_no_proxy_value;

    #[test]
    fn merge_no_proxy_adds_loopback_hosts() {
        assert_eq!(
            merge_no_proxy_value(None),
            "127.0.0.1,localhost,::1"
        );
    }

    #[test]
    fn merge_no_proxy_preserves_existing_entries() {
        assert_eq!(
            merge_no_proxy_value(Some("*.corp.example, localhost, github.com")),
            "127.0.0.1,localhost,::1,*.corp.example,github.com"
        );
    }
}
