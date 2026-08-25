use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

use mcpstore::config::ConfigManager;
use tracing_subscriber::EnvFilter;

static TRACING_INIT: Once = Once::new();
const RMCP_AUTH_LOG_DIRECTIVE: &str = "rmcp::transport::auth=info";

fn env_filter(default_directive: &str) -> EnvFilter {
    let rust_log = std::env::var("RUST_LOG").unwrap_or_default();
    env_filter_from(&rust_log, default_directive)
}

fn env_filter_from(rust_log: &str, default_directive: &str) -> EnvFilter {
    EnvFilter::new(rust_log)
        .add_directive(
            default_directive
                .parse()
                .expect("invalid tracing directive"),
        )
        // rmcp auth logs may include token exchange responses at debug level. Keep
        // that target above debug/trace even when RUST_LOG requests them.
        .add_directive(
            RMCP_AUTH_LOG_DIRECTIVE
                .parse()
                .expect("invalid rmcp auth tracing directive"),
        )
}

pub fn init_tracing(default_directive: &str) {
    TRACING_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(env_filter(default_directive))
            .with_target(true)
            .init();
    });
}

pub fn init_tracing_silent(default_directive: &str) {
    TRACING_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .with_env_filter(env_filter(default_directive))
            .with_target(true)
            .init();
    });
}

pub fn init_tracing_with_file(
    default_directive: &str,
    path: impl AsRef<Path>,
    max_size_bytes: u64,
    retention_days: Option<u64>,
) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        cleanup_old_logs(parent, &path, retention_days)?;
    }
    let writer = RotatingWriter::new(path, max_size_bytes)?;
    TRACING_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_writer(writer)
            .with_env_filter(env_filter(default_directive))
            .with_target(true)
            .init();
    });
    Ok(())
}

pub fn init_tracing_from_config(config: Option<&mcpstore::AppConfig>) {
    let default_path = ConfigManager::new()
        .mcp_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("logs")
        .join("mcpstore.log");
    init_tracing_from_config_with_path(config, default_path);
}

pub fn init_tracing_from_config_with_path(
    config: Option<&mcpstore::AppConfig>,
    log_path: std::path::PathBuf,
) {
    let (enabled, level, max_size_bytes, retention_days) = config
        .map(|c| {
            let rt = &c.diagnostics.runtime_log;
            (
                c.diagnostics.enabled && rt.enabled,
                rt.level.clone(),
                rt.max_size_bytes,
                rt.retention_days,
            )
        })
        .unwrap_or_else(|| {
            let rt = mcpstore::config::RuntimeLogConfig::default();
            (rt.enabled, rt.level, rt.max_size_bytes, rt.retention_days)
        });

    if enabled {
        if let Err(error) = init_tracing_with_file(
            &format!("mcpstore={}", level),
            log_path,
            max_size_bytes,
            retention_days,
        ) {
            // The subscriber is not installed yet, so report via stderr directly
            // instead of losing diagnostics silently.
            eprintln!("mcpstore: file logging unavailable ({error}); falling back to stderr");
            init_tracing(&format!("mcpstore={}", level));
        }
    } else {
        init_tracing("mcpstore=info");
    }
}

fn cleanup_old_logs(dir: &Path, current: &Path, retention_days: Option<u64>) -> io::Result<()> {
    let Some(days) = retention_days else {
        return Ok(());
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days.saturating_mul(86_400)))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let prefix = current
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path == current
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        {
            continue;
        }
        if entry
            .metadata()?
            .modified()
            .is_ok_and(|modified| modified < cutoff)
        {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[derive(Clone)]
struct RotatingWriter {
    path: PathBuf,
    max_size_bytes: u64,
    file: Arc<Mutex<File>>,
}

struct RotatingGuard {
    writer: RotatingWriter,
}

impl RotatingWriter {
    fn new(path: PathBuf, max_size_bytes: u64) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            max_size_bytes: max_size_bytes.max(1),
            file: Arc::new(Mutex::new(file)),
        })
    }

    fn rotate_if_needed(&self, file: &mut File) -> io::Result<()> {
        if file.metadata()?.len() < self.max_size_bytes {
            return Ok(());
        }
        let rotated = self.path.with_extension("log.1");
        let _ = std::fs::remove_file(&rotated);
        std::fs::rename(&self.path, rotated)?;
        *file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingWriter {
    type Writer = RotatingGuard;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingGuard {
            writer: self.clone(),
        }
    }
}

impl Write for RotatingGuard {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut file = self
            .writer
            .file
            .lock()
            .map_err(|_| io::Error::other("log file lock poisoned"))?;
        self.writer.rotate_if_needed(&mut file)?;
        file.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new()
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    struct CaptureGuard(Arc<Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureGuard;

        fn make_writer(&'a self) -> Self::Writer {
            CaptureGuard(Arc::clone(&self.0))
        }
    }

    impl Write for CaptureGuard {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn rmcp_auth_debug_logs_remain_disabled_when_rust_log_is_debug() {
        let writer = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer.clone())
            .with_env_filter(super::env_filter_from(
                "mcpstore_cli=debug,rmcp::transport::auth=trace",
                "info",
            ))
            .with_target(true)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(
                target: "rmcp::transport::auth",
                access_token = "secret-access-token",
                "token exchange response"
            );
            tracing::info!(target: "rmcp::transport::auth", "oauth lifecycle advanced");
            tracing::debug!(target: "mcpstore_cli::bootstrap", "ordinary debug remains enabled");
        });

        let output = String::from_utf8(writer.0.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("secret-access-token"));
        assert!(!output.contains("token exchange response"));
        assert!(output.contains("oauth lifecycle advanced"));
        assert!(output.contains("ordinary debug remains enabled"));
    }

    #[test]
    fn rotating_writer_creates_backup_when_exceeds_max_size() {
        use tracing_subscriber::fmt::MakeWriter as _;

        let dir = std::env::temp_dir().join(format!("mcpstore-rotate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("mcpstore.log");
        let writer = super::RotatingWriter::new(log_path.clone(), 64).unwrap();

        let mut first = writer.make_writer();
        first.write_all(&[b'a'; 96]).unwrap();
        drop(first);

        // Rotation is checked before each write, so the second write is what
        // moves the oversized file to the backup.
        let mut second = writer.make_writer();
        second.write_all(b"after rotation").unwrap();
        drop(second);

        let backup = log_path.with_extension("log.1");
        assert!(backup.exists(), "rotated backup should exist");
        assert_eq!(
            std::fs::metadata(&backup).unwrap().len(),
            96,
            "backup should hold the oversized content"
        );
        assert_eq!(
            std::fs::read(&log_path).unwrap(),
            b"after rotation".to_vec(),
            "main file should restart empty after rotation"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cleanup_old_logs_removes_expired_files() {
        let dir =
            std::env::temp_dir().join(format!("mcpstore-cleanup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let current = dir.join("mcpstore.log");
        std::fs::write(&current, "current").unwrap();
        let expired = dir.join("mcpstore.log.1");
        std::fs::write(&expired, "expired").unwrap();
        let unrelated = dir.join("other-service.log.1");
        std::fs::write(&unrelated, "keep me").unwrap();

        // retention_days = 0 makes the cutoff "now": any file whose mtime was
        // set strictly before this call counts as expired, which covers the
        // files written just above.
        super::cleanup_old_logs(&dir, &current, Some(0)).unwrap();

        assert!(current.exists(), "current log is never removed");
        assert!(!expired.exists(), "expired rotation should be deleted");
        assert!(
            unrelated.exists(),
            "files not sharing the log prefix are untouched"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_tracing_with_file_falls_back_to_stderr_when_path_unavailable() {
        let dir = std::env::temp_dir().join(format!(
            "mcpstore-init-fallback-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // A regular file where a directory is required makes create_dir_all
        // fail regardless of platform or privileges.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"file").unwrap();
        let log_path = blocker.join("nested").join("mcpstore.log");

        // Must not panic; init falls back to the stderr subscriber after the
        // eprintln above explains why file logging is off.
        super::init_tracing_from_config_with_path(None, log_path.clone());

        assert!(
            !log_path.exists(),
            "log file must not be created at an unavailable path"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
