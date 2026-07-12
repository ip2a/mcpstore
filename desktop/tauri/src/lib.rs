use anyhow::Result;
use mcpstore_cli::store_args::{build_store, StoreSourceArgs};
use std::collections::HashSet;
use std::ffi::OsString;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

// Mirrors `web/src/App.tsx` shell: w-[min(1280px,calc(100vw-24px))]
const WEB_CONTENT_MAX_WIDTH: f64 = 1280.0;
const WEB_CONTENT_HORIZONTAL_PADDING: f64 = 24.0;

/// Minimum width as a fraction of the 1280px content boundary (below the boundary is OK).
/// Effective min window width ≈ 1280 * ratio + 24. Tune while checking list/header layout.
const MIN_CONTENT_WIDTH_RATIO: f64 = 0.70;

/// Hard floor so ultra-narrow monitors still stay usable.
const ABSOLUTE_MIN_INNER_WIDTH: f64 = 720.0;

/// Height can shrink more aggressively than width; list pages scroll internally.
const DESIGN_MIN_INNER_HEIGHT: f64 = 480.0;

const DEFAULT_INNER_WIDTH: f64 = 1180.0;
const DEFAULT_INNER_HEIGHT: f64 = 760.0;

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

fn start_local_server() -> Result<String> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = std_listener.local_addr()?;
    std_listener.set_nonblocking(true)?;
    let url = format!("http://{}", addr);

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("desktop server runtime should build");
        runtime.block_on(async move {
            let store_args = StoreSourceArgs {
                config_path: None,
                source: mcpstore_cli::store_args::SourceArg::Local,
                backend: None,
                redis_url: None,
                namespace: None,
            };
            let store = match build_store(&store_args) {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("mcpstore desktop store setup failed: {error}");
                    return;
                }
            };
            if let Err(error) = store.load_from_source().await {
                eprintln!("mcpstore desktop store load failed: {error}");
                return;
            }
            let listener = tokio::net::TcpListener::from_std(std_listener)
                .expect("desktop listener should convert");
            let app = mcpstore_cli::commands::web::router(Arc::new(store));
            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("mcpstore desktop server stopped: {error}");
            }
        });
    });

    Ok(url)
}

fn logical_state_from_physical(size: PhysicalSize<u32>, scale_factor: f64) -> WindowState {
    WindowState {
        width: ((size.width as f64) / scale_factor).round().max(1.0) as u32,
        height: ((size.height as f64) / scale_factor).round().max(1.0) as u32,
    }
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
    let _ = window.set_min_size(Some(LogicalSize::new(
        min_size.width,
        min_size.height,
    )));
}

fn prime_desktop_environment() {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            prime_desktop_environment();
            if let Some(home) = dirs::home_dir() {
                let _ = std::env::set_current_dir(home);
            }

            let url = start_local_server()?;
            let initial_state = WindowState {
                width: DEFAULT_INNER_WIDTH as u32,
                height: DEFAULT_INNER_HEIGHT as u32,
            };
            let window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("desktop URL is valid")),
            )
            .title("mcpstore")
            .inner_size(initial_state.width as f64, initial_state.height as f64)
            .min_inner_size(ABSOLUTE_MIN_INNER_WIDTH, DESIGN_MIN_INNER_HEIGHT)
            .zoom_hotkeys_enabled(false)
            .center()
            .build()?;

            apply_min_inner_size(&window);

            let latest_window_state = Arc::new(Mutex::new(initial_state));
            let latest_window_state_handle = Arc::clone(&latest_window_state);
            let event_window = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Resized(size) => {
                    if let Ok(scale_factor) = event_window.scale_factor() {
                        if let Ok(mut state) = latest_window_state_handle.lock() {
                            *state = logical_state_from_physical(*size, scale_factor);
                        }
                    }
                }
                WindowEvent::Moved(_) => {
                    apply_min_inner_size(&event_window);
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                    ..
                } => {
                    apply_min_inner_size(&event_window);
                    if let Ok(mut state) = latest_window_state_handle.lock() {
                        *state = logical_state_from_physical(*new_inner_size, *scale_factor);
                    }
                }
                _ => {}
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mcpstore tauri app");
}
