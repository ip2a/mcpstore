use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mcpstore::config::ConfigManager;
use serde::{Deserialize, Serialize};

use mcpstore::error::{Error, FailureCode};

use crate::store_args::StoreSourceArgs;

#[derive(Serialize, Deserialize)]
struct DaemonStartState {
    store: StoreSourceArgs,
}

/// Probe whether the daemon is ready: the socket connects and the handshake passes.
pub async fn is_daemon_ready() -> bool {
    crate::daemon::client::connect_admin(None).await.is_ok()
}

/// Wait for the daemon: 200ms polling; on timeout, error and suggest a foreground run.
pub async fn wait_daemon_ready(timeout: Duration) -> Result<(), Error> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if is_daemon_ready().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(Error::new(
        FailureCode::ServiceUnavailable,
        "daemon 未在期限内就绪；运行 mcpstore start（前台）排查原因",
    ))
}

/// Spawn the daemon in the background: detached, own process group, stdout/stderr appended to logs/daemon.out in the config dir.
pub fn spawn_detached_daemon() -> io::Result<()> {
    use std::process::{Command, Stdio};

    let restart_args = restore_start_args()?;

    let log_path = daemon_log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let exe = std::env::current_exe()?;
    let mut command = Command::new(exe);
    command.arg("start");
    if let Some(args) = restart_args {
        append_start_args(&mut command, &args);
    }
    command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group: detaches from the current terminal so the parent (CLI) exiting doesn't affect the daemon.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn()?;
    Ok(())
}

pub fn persist_start_args(args: &StoreSourceArgs) -> io::Result<()> {
    let state = DaemonStartState {
        store: args.clone(),
    };
    write_start_args(&default_runtime_path(), &state)
}

fn restore_start_args() -> io::Result<Option<StoreSourceArgs>> {
    let path = default_runtime_path();
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    let state: DaemonStartState = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(state.store))
}

fn write_start_args(path: &Path, state: &DaemonStartState) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, bytes)
}

fn append_start_args(command: &mut std::process::Command, args: &StoreSourceArgs) {
    if let Some(path) = &args.config_path {
        command.arg("--config-path").arg(path);
    }
    command.arg("--source").arg(args.source.as_str());
    if let Some(store) = &args.store {
        command.arg("--store").arg(store);
    }
    if let Some(config) = &args.store_config {
        command.arg("--store-config").arg(config);
    }
    if let Some(namespace) = &args.namespace {
        command.arg("--namespace").arg(namespace);
    }
    // Control panel is the default; replay only the data-panel flags so a detached restart doesn't fall back to the default shape.
    if args.data_panel {
        command.arg("--data-panel");
    }
    if let Some(panel_id) = &args.panel_id {
        command.arg("--panel-id").arg(panel_id);
    }
}

fn default_runtime_path() -> PathBuf {
    crate::daemon::protocol::default_pid_path().with_extension("runtime.json")
}

fn daemon_log_path() -> PathBuf {
    ConfigManager::new()
        .mcp_path()
        .parent()
        .map(|parent| parent.join("logs").join("daemon.out"))
        .unwrap_or_else(|| PathBuf::from("logs/daemon.out"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_state_round_trips_store_arguments() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-daemon-state-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state = DaemonStartState {
            store: StoreSourceArgs {
                config_path: Some("/tmp/mcp.json".into()),
                source: crate::store_args::SourceArg::Db,
                store: Some("redis".into()),
                store_config: Some(r#"{"url":"redis://127.0.0.1"}"#.into()),
                namespace: Some("tenant-a".into()),
                control_panel: false,
                data_panel: false,
                panel_id: Some("worker-1".into()),
            },
        };

        write_start_args(&path, &state).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let restored: DaemonStartState = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(restored.store.config_path.as_deref(), Some("/tmp/mcp.json"));
        assert_eq!(restored.store.store.as_deref(), Some("redis"));
        assert_eq!(restored.store.namespace.as_deref(), Some("tenant-a"));
        assert_eq!(restored.store.panel_id.as_deref(), Some("worker-1"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn append_start_args_replays_explicit_data_panel() {
        let args = StoreSourceArgs {
            config_path: None,
            source: crate::store_args::SourceArg::Db,
            store: Some("redis".into()),
            store_config: None,
            namespace: None,
            control_panel: false,
            data_panel: true,
            panel_id: None,
        };
        let mut command = std::process::Command::new("mcpstore");
        append_start_args(&mut command, &args);
        let argv: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(argv.contains(&"--data-panel".to_string()));
        assert!(!argv.contains(&"--control-panel".to_string()));
    }
}
