use clap::Args;
use serde_json::json;

use crate::daemon::protocol::{default_pid_path, is_daemon_running};
use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

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
    let pid: u32 = pid_str.trim().parse()?;

    // Try graceful stop via socket first.
    match crate::daemon::client::call_daemon("stop_daemon", json!({})).await {
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
                    .arg(pid.to_string())
                    .status()?;
                println!("[Success] Daemon killed (pid={}).", pid);
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
