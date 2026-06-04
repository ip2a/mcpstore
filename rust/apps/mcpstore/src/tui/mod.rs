
use clap::Parser;

use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

pub mod app;
pub mod events;
pub mod ui;
pub mod widgets;

#[derive(Parser)]
#[command(
    name = "mcpstore-tui",
    about = "MCPStore 终端服务管理界面",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct TuiArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, default_value_t = 250, help = "事件轮询间隔（毫秒）")]
    pub tick_ms: u64,
}

pub fn run() -> Result<(), BoxErr> {
    let args = TuiArgs::parse();
    app::run(&args.store, args.tick_ms)
}
