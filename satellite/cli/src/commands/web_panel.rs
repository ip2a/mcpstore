//! The standalone web panel process: static frontend + panel API under one
//! same-origin port. No embedded store — every daemon, including the local
//! one, is reached over HTTP (hosts table; "local" is the default target).

use std::sync::Arc;

use mcpstore::config::ConfigManager;

use crate::commands::{api, web_assets};

/// Bind address precedence: CLI flag → config.toml `[web]` → default.
pub async fn run(
    host: Option<String>,
    port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_manager = Arc::new(ConfigManager::new());
    let web = config_manager
        .load_app_config_or_default()
        .map(|config| config.web)
        .unwrap_or_default();
    let host = host.unwrap_or(web.host);
    let port = port.unwrap_or(web.port);

    api::adopt_panel_settings(&config_manager)?;
    let state = Arc::new(api::PanelState::new(Arc::clone(&config_manager)));

    // Auth is an explicit opt-in (`[web] auth = true`); a loopback bind adds
    // the foreign-Host-header guard (DNS rebinding) regardless. A bind address
    // that fails to resolve counts as non-loopback — fail loud.
    let resolves_loopback = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map(|mut addrs| addrs.all(|addr| addr.ip().is_loopback()))
        .unwrap_or(false);
    let auth_guard =
        api::panel_auth::PanelAuthGuard::for_bind(resolves_loopback, web.auth, &config_manager)?;

    // Same-origin /api (panel routes + forwarding) and the static frontend,
    // mirroring the daemon's web face layout.
    let config = Arc::new(config_manager.load_app_config_or_default()?);
    let router = axum::Router::new()
        .nest(
            "/api",
            api::panel_router(state).layer(axum::middleware::from_fn_with_state(
                auth_guard.clone(),
                api::panel_auth::enforce,
            )),
        )
        .fallback(web_assets::serve_react_app)
        .layer(axum::middleware::from_fn(
            move |mut request: axum::extract::Request, next: axum::middleware::Next| {
                let config = Arc::clone(&config);
                async move {
                    request.extensions_mut().insert(config);
                    next.run(request).await
                }
            },
        ));

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    let url = format!("http://{host}:{port}");
    println!("mcpstore web panel serving on {url}");
    if auth_guard.bearer_enabled() {
        println!(
            "  /api requires the bearer token from {}",
            api::panel_auth::PanelAuthGuard::token_path(&config_manager).display()
        );
    } else if resolves_loopback {
        println!("  bind is loopback; no token required, foreign Host headers rejected");
    } else {
        println!(
            "  WARNING: bind is non-loopback and panel auth is disabled; /api is unauthenticated (set [web] auth = true in {} to enable)",
            config_manager.app_config_path().display()
        );
    }
    println!(
        "  local daemon: the 'local' entry of {}",
        api::panel_settings_path(&config_manager).display()
    );

    axum::serve(listener, router).await?;
    Ok(())
}
