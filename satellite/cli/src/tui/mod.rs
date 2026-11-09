use clap::Parser;

use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

pub mod wizard;

#[derive(Parser)]
#[command(
    name = "mcpstore-tui",
    about = "MCPStore interactive terminal mode (zi style inline prompts)",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct TuiArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, help = "cold-start an embedded kernel in this process instead of connecting to the daemon")]
    pub embedded: bool,
}

pub fn run() -> Result<(), BoxErr> {
    let args = TuiArgs::parse();
    run_from_args(&args)
}

pub fn run_from_args(args: &TuiArgs) -> Result<(), BoxErr> {
    wizard::run(&args.store, args.embedded)
}
