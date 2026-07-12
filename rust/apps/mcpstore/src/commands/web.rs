use clap::Args;
use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
    Router,
};

use crate::{
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

#[derive(Args)]
pub struct WebArgs {
    #[arg(long, default_value_t = 8080, help = "Web UI 端口")]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn run(args: WebArgs) -> Result<(), BoxErr> {
    let store = build_store(&args.store)?;
    store.load_from_source().await?;

    let app = router(Arc::new(store));

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("[Web UI] Starting at http://{}/", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router(store: Arc<mcpstore::MCPStore>) -> Router {
    let api = crate::commands::api::router_for_store(store, "/api");
    Router::new()
        .merge(api)
        .route("/api", any(api_not_found))
        .route("/api/*path", any(api_not_found))
        .fallback(serve_react_app)
}

async fn serve_react_app(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    let asset_path = if path.is_empty() { "index.html" } else { path };

    if crate::commands::web_assets::has_override_assets() && asset_path == "index.html" {
        return crate::commands::web_assets::response_for_asset(asset_path);
    }

    if crate::commands::web_assets::find(asset_path).is_some() {
        return crate::commands::web_assets::response_for_asset(asset_path);
    }

    if !crate::commands::web_assets::has_assets() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "mcpstore web assets are missing. Run `cd web && npm run build` before building or set MCPSTORE_WEB_ASSETS_DIR.",
        )
            .into_response();
    }

    if has_file_extension(asset_path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    crate::commands::web_assets::response_for_asset("index.html")
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

fn has_file_extension(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some()
}
