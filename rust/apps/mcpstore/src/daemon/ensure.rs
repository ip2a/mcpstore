use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mcpstore::config::ConfigManager;
use mcpstore::error::{Error, FailureCode};

/// 探测 daemon 是否就绪：socket 可连且握手通过。
pub async fn is_daemon_ready() -> bool {
    crate::daemon::client::connect_admin().await.is_ok()
}

/// 等待 daemon 就绪，200ms 轮询，超时报错并提示前台排查。
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

/// 后台拉起 daemon：detached、独立进程组，stdout/stderr 追加到配置目录 logs/daemon.out。
pub fn spawn_detached_daemon() -> io::Result<()> {
    use std::process::{Command, Stdio};

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
    command
        .arg("start")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // 独立进程组：脱离当前终端，父进程（CLI）退出不影响 daemon。
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

fn daemon_log_path() -> PathBuf {
    ConfigManager::new()
        .mcp_path()
        .parent()
        .map(|parent| parent.join("logs").join("daemon.out"))
        .unwrap_or_else(|| PathBuf::from("logs/daemon.out"))
}
