use clap::Args;
use std::sync::Arc;

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

    let app = crate::commands::ui_web::router(Arc::new(store));

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("[Web UI] Starting at http://{}/", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
