//! MCPStore Event Bus
//!
//! Async publish/subscribe event system migrated from Python core/events/.
//! Features:
//! - Priority subscriptions (higher priority executed first)
//! - Error isolation (one handler failure does not affect others)
//! - Optional per-handler timeout
//! - Critical event whitelist (forces synchronous dispatch)
//! - Optional capped event history
//!
//! P2 priority. Replaces Python asyncio queue with tokio channels/tasks.

pub mod bus;
mod event;
mod event_bus;
mod store;
#[cfg(test)]
mod tests;
pub mod types;

pub use event::Event;
pub use event_bus::EventBus;
pub use types::EventCapabilityReport;
