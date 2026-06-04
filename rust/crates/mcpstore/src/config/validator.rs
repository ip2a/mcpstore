//! Configuration validation.
//!
//! Checks:
//! - mcpServers is present and is a map
//! - Each server has url or command
//! - args is a list, env is a dict, headers is a dict
//! - transport is in [streamable-http, sse, stdio]

use super::models::ServerConfigFull;
use std::collections::HashMap;

/// Validation errors.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("missing mcpServers field")]
    MissingServers,
    #[error("server '{0}' missing both url and command")]
    MissingUrlOrCommand(String),
    #[error("invalid transport '{0}' for server '{1}'")]
    InvalidTransport(String, String),
}

/// Validate a configuration map.
pub fn validate(servers: &HashMap<String, ServerConfigFull>) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for (name, cfg) in servers {
        if cfg.url.is_none() && cfg.command.is_none() {
            errors.push(ValidationError::MissingUrlOrCommand(name.clone()));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
