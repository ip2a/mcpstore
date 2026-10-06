// Deeply nested async blocks in cli_app overflow rustc's default query depth
// on newer stable toolchains (seen in CI); this is rustc's own suggested fix.
#![recursion_limit = "256"]

pub mod bootstrap;
pub mod cli_app;
pub mod commands;
pub mod daemon;
pub mod error;
pub mod mcp_server;
pub mod schema_cache;
pub mod store_args;
pub mod tui;

pub type BoxErr = Box<dyn std::error::Error>;
