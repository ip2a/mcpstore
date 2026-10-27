use anyhow::Result;
use mcpstore_cli::daemon::listeners::web_router;
use mcpstore_cli::store_args::StoreSourceArgs;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::process::Command;
use tauri::{LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

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
                store: None,
                store_config: None,
                namespace: None,
                control_panel: false,
                data_panel: false,
                panel_id: None,
            };
            let access = match mcpstore_cli::store_args::open_store_access(&store_args, true, None).await
            {
                Ok(a) => a,
                Err(error) => {
                    eprintln!("mcpstore desktop store setup failed: {error}");
                    return;
                }
            };
            let store = match access.embedded_store() {
                Some(s) => std::sync::Arc::clone(s),
                None => {
                    eprintln!("mcpstore desktop expected embedded store");
                    return;
                }
            };
            let listener = tokio::net::TcpListener::from_std(std_listener)
                .expect("desktop listener should convert");
            let state = mcpstore_cli::commands::api::state_for_store(store);
            let app = web_router(state);
            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("mcpstore desktop server stopped: {error}");
            }
        });
    });

    Ok(url)
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

#[derive(Debug, Serialize, Deserialize)]
struct ProxyResponse {
    status: u16,
    body: String,
    headers: HashMap<String, String>,
}

/// Tauri command: 代理 API 请求到配置的后端，绕过浏览器 CORS 限制
#[tauri::command]
async fn proxy_api_request(
    method: String,
    path: String,
    body: Option<String>,
    headers: Option<HashMap<String, String>>,
    target_base: String,
) -> Result<ProxyResponse, String> {
    let url = format!("{}{}", target_base.trim_end_matches('/'), path);
    let client = reqwest::Client::new();

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        _ => return Err(format!("Unsupported method: {}", method)),
    };

    if let Some(headers_map) = headers {
        for (key, value) in headers_map {
            request = request.header(key, value);
        }
    }

    if let Some(body_str) = body {
        request = request.body(body_str);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status().as_u16();
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|val| (k.as_str().to_string(), val.to_string()))
        })
        .collect();

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    Ok(ProxyResponse {
        status,
        body,
        headers: response_headers,
    })
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
        .invoke_handler(tauri::generate_handler![proxy_api_request])
        .setup(|app| {
            prime_desktop_environment();
            if let Some(home) = dirs::home_dir() {
                let _ = std::env::set_current_dir(home);
            }

            // First launch only: materialize the bundled plugins that match
            // agents on this machine, then never read the bundle again. A
            // missing or broken bundle means zero materialized plugins — the
            // agents page can still install from the market.
            if let Some(bundle) = app
                .path()
                .resource_dir()
                .ok()
                .map(|dir| dir.join("plugins"))
            {
                match mcpstore::agents::registry::materialize_bundle(&bundle) {
                    Ok(count) => eprintln!("mcpstore desktop materialized {count} bundled plugin(s)"),
                    Err(error) => eprintln!("mcpstore desktop plugin materialization failed: {error}"),
                }
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
            .hidden_title(true)
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
