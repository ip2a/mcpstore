use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

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
        // rmcp 2.2 emits token exchange responses at debug level. Keep that target
        // above debug/trace even when the process-wide RUST_LOG requests them.
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
            .with_target(false)
            .init();
    });
}

pub fn init_tracing_silent(default_directive: &str) {
    TRACING_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .with_env_filter(env_filter(default_directive))
            .with_target(false)
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
            .with_target(false)
            .init();
    });
    Ok(())
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
}
