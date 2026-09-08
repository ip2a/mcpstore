use clap::Args;
use std::sync::Arc;

use axum::{
    http::StatusCode,
    routing::any,
    Router,
};

use crate::{
    store_args::{load_kernel, StoreSourceArgs},
    BoxErr,
};

#[derive(Args)]
pub struct WebArgs {
    #[arg(long, help = "Web UI 端口；未指定时读取 app 配置")]
    pub port: Option<u16>,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn run(args: WebArgs) -> Result<(), BoxErr> {
    let store = load_kernel(&args.store).await?.store().clone();
    let config = store.config_manager().load_app_config_or_default()?;
    let port = args.port.unwrap_or(config.server.web_port);

    let app = router(store);

    let addr = format!("{}:{}", args.host, port);
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
        .fallback(crate::commands::web_assets::serve_react_app)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
