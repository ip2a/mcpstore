use clap::{Parser, ValueEnum};

use self::i18n::Locale;
use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

pub mod app;
pub mod events;
pub mod i18n;
pub mod layout;
pub mod pages;
pub mod theme;
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
    #[arg(long, value_enum, help = "TUI language")]
    pub locale: Option<LocaleArg>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LocaleArg {
    ZhCn,
    EnUs,
}

impl From<LocaleArg> for Locale {
    fn from(value: LocaleArg) -> Self {
        match value {
            LocaleArg::ZhCn => Locale::ZhCn,
            LocaleArg::EnUs => Locale::EnUs,
        }
    }
}

pub fn run() -> Result<(), BoxErr> {
    let args = TuiArgs::parse();
    run_from_args(&args)
}

pub fn run_from_args(args: &TuiArgs) -> Result<(), BoxErr> {
    app::run(&args.store, args.tick_ms, args.locale.map(Into::into))
}
