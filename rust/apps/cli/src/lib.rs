pub mod bootstrap;
pub mod cli_app;
pub mod commands;
pub mod daemon;
pub mod store_args;
pub mod tui;

pub type BoxErr = Box<dyn std::error::Error>;
