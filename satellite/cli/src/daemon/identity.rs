use std::path::Path;

use base64::Engine;
use rand::RngCore;

const DAEMON_ID_FILENAME: &str = "daemon-id";

/// Generate a unique daemon identifier (学 Paseo server-id).
/// Format: `d_` + 9 bytes base64url (12 chars) = 14 chars total
fn generate_daemon_id() -> String {
    let mut bytes = [0u8; 9];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!(
        "d_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

/// Get or create the daemon ID for this data directory.
///
/// - Checks `MCPSTORE_DAEMON_ID` env var first (test override, 学 Paseo)
/// - Reads existing `daemon-id` file if present
/// - Generates new ID on first run
/// - Enforces 0600 permissions
pub fn get_or_create_daemon_id(data_dir: &Path) -> Result<String, String> {
    let path = data_dir.join(DAEMON_ID_FILENAME);

    // Environment variable override (for tests)
    if let Ok(env_id) = std::env::var("MCPSTORE_DAEMON_ID") {
        let env_id = env_id.trim();
        if !env_id.is_empty() {
            if !path.exists() {
                atomic_write(&path, env_id.as_bytes())?;
                set_permissions_0600(&path)?;
                tracing::info!("Persisted MCPSTORE_DAEMON_ID override: {}", env_id);
            }
            return Ok(env_id.to_string());
        }
    }

    // Existing file
    if path.exists() {
        ensure_permissions_0600(&path)?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read daemon-id: {}", e))?
            .trim()
            .to_string();
        if content.is_empty() {
            return Err("daemon-id file is empty".to_string());
        }
        return Ok(content);
    }

    // Generate new ID
    let id = generate_daemon_id();
    atomic_write(&path, id.as_bytes())?;
    set_permissions_0600(&path)?;
    tracing::info!("Generated daemon ID: {}", id);
    Ok(id)
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    file.write_all(content).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(unix)]
fn set_permissions_0600(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Failed to set 0600 permissions: {}", e))
}

#[cfg(not(unix))]
fn set_permissions_0600(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn ensure_permissions_0600(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        tracing::warn!(
            "daemon-id has unsafe permissions {:o}, fixing to 0600",
            mode
        );
        set_permissions_0600(path)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_permissions_0600(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "daemon-id-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    /// `get_or_create_daemon_id` reads the process env; sibling tests that mutate
    /// `MCPSTORE_DAEMON_ID` race with these tests under parallel execution.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn generates_14_char_id() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("generate");
        let id = get_or_create_daemon_id(&dir).unwrap();
        // `d_` + 9 bytes base64url = 2 + 12 = 14 chars total
        assert_eq!(id.len(), 14);
        assert!(id.starts_with("d_"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn persists_and_reuses_id() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("persist");
        let id1 = get_or_create_daemon_id(&dir).unwrap();
        let id2 = get_or_create_daemon_id(&dir).unwrap();
        assert_eq!(id1, id2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn env_override_is_persisted() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("env");
        env::set_var("MCPSTORE_DAEMON_ID", "d_test123456");
        let id = get_or_create_daemon_id(&dir).unwrap();
        assert_eq!(id, "d_test123456");
        env::remove_var("MCPSTORE_DAEMON_ID");
        let id2 = get_or_create_daemon_id(&dir).unwrap();
        assert_eq!(id2, "d_test123456");
        std::fs::remove_dir_all(&dir).ok();
    }
}
