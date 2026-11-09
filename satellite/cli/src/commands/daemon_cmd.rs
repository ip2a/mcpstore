use crate::daemon::protocol::{default_pid_path, is_daemon_running};
use crate::store_args::StoreSourceArgs;
use crate::BoxErr;
use clap::Args;
use mcpstore::config::ConfigManager;
use serde_json::{json, Value};

#[derive(Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn start(args: StartArgs) -> Result<(), BoxErr> {
    if is_daemon_running() {
        println!("[Warning] Daemon is already running.");
        return Ok(());
    }

    crate::daemon::protocol::cleanup_stale_files();

    // First launch: materialize bundled plugins for detected clients.
    // Try common bundle locations: development repo or installed resources.
    let mut bundle_candidates = Vec::new();

    // Development: relative to current directory (repo root or satellite/).
    if let Ok(cwd) = std::env::current_dir() {
        bundle_candidates.push(cwd.join("satellite/plugins"));
        bundle_candidates.push(cwd.join("plugins"));
        if let Some(parent) = cwd.parent() {
            bundle_candidates.push(parent.join("satellite/plugins"));
            bundle_candidates.push(parent.join("plugins"));
        }
    }

    for bundle in bundle_candidates {
        if bundle.is_dir() {
            match mcpstore::agents::registry::materialize_bundle(&bundle) {
                Ok(count) if count > 0 => {
                    println!(
                        "[Plugin] Materialized {count} bundled plugin(s) from {}",
                        bundle.display()
                    );
                    break;
                }
                Ok(_) => {} // Already materialized
                Err(error) => {
                    eprintln!(
                        "[Plugin] Materialization from {} failed: {error}",
                        bundle.display()
                    );
                }
            }
            break;
        }
    }

    // Run the daemon in the foreground (user can background with `&` or systemd).
    crate::daemon::server::start_daemon(args.store).await?;
    Ok(())
}

pub async fn stop() -> Result<(), BoxErr> {
    if !is_daemon_running() {
        println!("[Warning] Daemon is not running.");
        return Ok(());
    }

    let pid_path = default_pid_path();
    let pid_str = std::fs::read_to_string(&pid_path)?;
    let _pid: u32 = pid_str.trim().parse()?;

    // Try graceful stop via socket first.
    match crate::daemon::client::stop_daemon().await {
        Ok(_) => {
            println!("[Success] Daemon stop requested.");
        }
        Err(_) => {
            // Fallback: kill the process.
            #[cfg(unix)]
            {
                use std::process::Command;
                Command::new("kill")
                    .arg("-TERM")
                    .arg(_pid.to_string())
                    .status()?;
                println!("[Success] Daemon killed (pid={}).", _pid);
            }
            #[cfg(not(unix))]
            {
                println!("[Error] Cannot stop daemon on non-Unix platform.");
            }
        }
    }

    // Clean up stale files.
    let _ = std::fs::remove_file(pid_path);
    let _ = std::fs::remove_file(crate::daemon::protocol::default_socket_path());
    Ok(())
}

#[derive(clap::Subcommand)]
pub enum DaemonAction {
    /// Restart the daemon in the background after a graceful stop (distinct from the service-level `mcpstore restart SERVICE`)
    Restart,
}

pub async fn run_daemon(action: DaemonAction) -> Result<(), BoxErr> {
    match action {
        DaemonAction::Restart => restart().await,
    }
}

/// Re-spawn in the background after a graceful stop, then wait for readiness.
pub async fn restart() -> Result<(), BoxErr> {
    if is_daemon_running() {
        stop().await?;
    }
    crate::daemon::ensure::spawn_detached_daemon()?;
    crate::daemon::ensure::wait_daemon_ready(std::time::Duration::from_secs(30)).await?;
    println!("[Success] Daemon restarted.");
    Ok(())
}

/// Daemon status overview: when the socket is unreachable, say not running plainly — never spawn.
pub async fn status(json: bool) -> Result<(), BoxErr> {
    if !is_daemon_running() {
        if json {
            println!("{}", json!({"running": false}));
        } else {
            println!("[Info] Daemon is not running.");
        }
        return Ok(());
    }
    let mut client = match crate::daemon::client::connect_admin(None).await {
        Ok(client) => client,
        Err(error) => {
            println!("[Warning] Daemon process exists but socket unreachable: {error}");
            return Ok(());
        }
    };
    let status = client.status_host().await?;
    if json {
        println!("{status}");
        return Ok(());
    }
    println!(
        "[Success] Daemon running (pid={}, version={}, uptime={}s, namespace={})",
        status["pid"], status["version"], status["uptime_s"], status["namespace"],
    );
    for listener in status["listeners"].as_array().into_iter().flatten() {
        let key = listener["key"].as_str().unwrap_or("?");
        match listener["bind"].as_str() {
            Some(bind) => println!("  {key}: http://{bind}"),
            None => println!("  {key}: off"),
        }
    }
    Ok(())
}

/// Bare `mcpstore api` / `mcpstore web`: read-only view plus a hint to edit; never starts any process.
pub async fn face_view(face: &str, json: bool) -> Result<(), BoxErr> {
    let tip = match face {
        "core" => "mcpstore config --core-port <port> | mcpstore config --core <on|off> | mcpstore config --host <ip>",
        _ => "mcpstore config --web-port <port> | mcpstore config --web <on|off> | mcpstore config --host <ip>",
    };
    let mut client = if is_daemon_running() {
        crate::daemon::client::connect_admin(None).await.ok()
    } else {
        None
    };
    match client.as_mut() {
        Some(client) => {
            let status = client.status_host().await?;
            let config = client.get_daemon_config().await?;
            let bind = status["listeners"]
                .as_array()
                .and_then(|listeners| {
                    listeners
                        .iter()
                        .find(|listener| listener["key"] == face)
                        .map(|listener| listener["bind"].clone())
                })
                .unwrap_or(Value::Null);
            if json {
                println!(
                    "{}",
                    json!({"face": face, "running": true, "pid": status["pid"], "bind": bind, "server": config["config"]["server"]})
                );
            } else {
                let bind = bind
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| "off".to_string());
                println!("[{face}] daemon: running (pid={})", status["pid"]);
                println!("[{face}] bind: {bind}");
                println!("Use this command to change settings: {tip}");
            }
        }
        None => {
            let config = ConfigManager::new().load_app_config_or_default()?;
            let (enabled, port) = (config.server.core_enabled, config.server.port);
            if json {
                println!(
                    "{}",
                    json!({"face": face, "running": false, "enabled": enabled, "port": port, "host": config.server.host})
                );
            } else {
                println!("[{face}] daemon: not running (showing values from config.toml)");
                println!(
                    "[{face}] enabled: {}  host: {}  port: {}",
                    if enabled { "on" } else { "off" },
                    config.server.host,
                    port
                );
                println!("Use this command to change settings: {tip}");
            }
        }
    }
    Ok(())
}
