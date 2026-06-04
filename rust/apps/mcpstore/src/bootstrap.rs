use std::sync::Once;

static TRACING_INIT: Once = Once::new();

pub fn init_tracing(default_directive: &str) {
    TRACING_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env().add_directive(
                    default_directive
                        .parse()
                        .expect("invalid tracing directive"),
                ),
            )
            .with_target(false)
            .init();
    });
}

pub fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new()
}
