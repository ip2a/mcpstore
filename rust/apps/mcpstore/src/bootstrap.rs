use std::sync::Once;

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
