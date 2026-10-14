use clap::Parser;

use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

pub mod wizard;

#[derive(Parser)]
#[command(
    name = "mcpstore-tui",
    about = "MCPStore 终端交互模式（zi 风格内联 prompt）",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct TuiArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, help = "本进程内嵌 kernel 冷启动，不连 daemon")]
    pub embedded: bool,
}

pub fn run() -> Result<(), BoxErr> {
    let args = TuiArgs::parse();
    run_from_args(&args)
}

pub fn run_from_args(args: &TuiArgs) -> Result<(), BoxErr> {
    wizard::run(&args.store, args.embedded)
}
