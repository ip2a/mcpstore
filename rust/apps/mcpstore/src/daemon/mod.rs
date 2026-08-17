#[cfg(unix)]
pub mod client;
pub mod protocol;
#[cfg(unix)]
pub mod server;

#[cfg(not(unix))]
pub mod client {
    use serde_json::Value;

    pub fn daemon_socket_exists() -> bool {
        false
    }

    pub async fn call_daemon(_method: impl Into<String>, _params: Value) -> Result<Value, String> {
        Err("Daemon mode is only available on Unix platforms.".to_string())
    }
}

#[cfg(not(unix))]
pub mod server {
    use crate::store_args::StoreSourceArgs;

    pub async fn start_daemon(_args: StoreSourceArgs) -> Result<(), Box<dyn std::error::Error>> {
        Err("Daemon mode is only available on Unix platforms.".into())
    }
}
