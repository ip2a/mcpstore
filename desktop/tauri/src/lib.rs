use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::{
    AppHandle, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

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

fn daemon_web_url(app: &AppHandle) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        if !mcpstore_cli::daemon::ensure::is_daemon_ready().await {
            start_daemon(app)?;
            mcpstore_cli::daemon::ensure::wait_daemon_ready(Duration::from_secs(30)).await?;
        }

        let client = Client::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let mut admin = match mcpstore_cli::daemon::client::connect_admin(None).await {
                Ok(client) => client,
                Err(error) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let _ = error;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let status = admin.status_host().await?;
            if let Some(bind) = status["listeners"].as_array().and_then(|listeners| {
                listeners.iter().find_map(|listener| {
                    (listener["key"] == "web" && listener["running"] == true)
                        .then(|| listener["bind"].as_str())
                        .flatten()
                })
            }) {
                let url = format!("http://{bind}");
                if client.get(format!("{url}/api/health")).send().await.is_ok() {
                    return Ok(url);
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!("daemon web face did not become ready"));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
}

fn start_daemon(app: &AppHandle) -> Result<Child> {
    // tauri-build copies the externalBin sidecar (built as
    // binaries/mcpstore-<target-triple>) next to the compiled app binary with
    // the triple stripped: target/debug/mcpstore in dev, Contents/MacOS/mcpstore
    // in a packaged .app. It does NOT live under resource_dir().
    let executable = std::env::current_exe()?
        .parent()
        .map(|dir| {
            dir.join(if cfg!(windows) {
                "mcpstore.exe"
            } else {
                "mcpstore"
            })
        })
        .filter(|path| path.is_file())
        .ok_or_else(|| anyhow!("packaged mcpstore daemon binary is missing"))?;
    // Bundled plugins ship as Resources/plugins; the daemon's own first-launch
    // materialization scans cwd/plugins, so run it from resource_dir().
    let resource_dir = app.path().resource_dir()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("home directory is unavailable"))?;
    let log_path = home
        .join(".mcpstore")
        .join("logs")
        .join("desktop-daemon.out");
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    Ok(Command::new(executable)
        .arg("start")
        .current_dir(resource_dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?)
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

            let url = daemon_web_url(&app.handle())?;
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
