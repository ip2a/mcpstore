//! Agent shell registry: agent scopes that exist before any service is declared.
//!
//! Persisted next to the app config as `agent-shells.json`. Distinct from
//! `client-agents.json` (client→agent wiring). Both feed `list_scopes` so empty
//! agent scopes remain visible.

use crate::{Error, FailureCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentShell {
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentShellRegistry {
    #[serde(default)]
    pub shells: HashMap<String, AgentShell>,
}

pub fn validate_agent_shell_id(agent_id: &str) -> Result<()> {
    let trimmed = agent_id.trim();
    if trimmed.is_empty() {
        return Err(Error::new(
            FailureCode::ConfigInvalid,
            "Agent ID cannot be empty",
        ));
    }
    if trimmed.eq_ignore_ascii_case("all") || trimmed.eq_ignore_ascii_case("store") {
        return Err(Error::new(
            FailureCode::ConfigInvalid,
            format!("Agent ID '{trimmed}' is reserved"),
        ));
    }
    Ok(())
}

pub fn load_agent_shell_registry(path: &Path) -> Result<AgentShellRegistry> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to parse {}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(AgentShellRegistry::default())
        }
        Err(error) => Err(Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", path.display()),
        )),
    }
}

pub fn save_agent_shell_registry(path: &Path, registry: &AgentShellRegistry) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to create {}: {error}", parent.display()),
            )
        })?;
    }
    let body = serde_json::to_vec_pretty(registry)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    let temp = path.with_extension("mcpstore-tmp");
    fs::write(&temp, body).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to write {}: {error}", temp.display()),
        )
    })?;
    fs::rename(&temp, path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to rename {}: {error}", path.display()),
        )
    })
}

/// Idempotent: create shell if missing, otherwise leave as-is.
pub fn ensure_agent_shell(registry_path: &Path, agent_id: &str) -> Result<()> {
    validate_agent_shell_id(agent_id)?;
    let agent_id = agent_id.trim();
    let mut registry = load_agent_shell_registry(registry_path)?;
    if registry.shells.contains_key(agent_id) {
        return Ok(());
    }
    registry.shells.insert(
        agent_id.to_string(),
        AgentShell {
            created_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        },
    );
    save_agent_shell_registry(registry_path, &registry)
}

/// Remove a shell registration. Does not touch declared service instances.
pub fn remove_agent_shell(registry_path: &Path, agent_id: &str) -> Result<bool> {
    validate_agent_shell_id(agent_id)?;
    let agent_id = agent_id.trim();
    let mut registry = load_agent_shell_registry(registry_path)?;
    let removed = registry.shells.remove(agent_id).is_some();
    if removed {
        save_agent_shell_registry(registry_path, &registry)?;
    }
    Ok(removed)
}

pub fn agent_shell_exists(registry_path: &Path, agent_id: &str) -> Result<bool> {
    let registry = load_agent_shell_registry(registry_path)?;
    Ok(registry.shells.contains_key(agent_id.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_and_remove_shell_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-agent-shell-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_file(&path);

        ensure_agent_shell(&path, "claude").unwrap();
        ensure_agent_shell(&path, "claude").unwrap();
        assert!(agent_shell_exists(&path, "claude").unwrap());

        let registry = load_agent_shell_registry(&path).unwrap();
        assert_eq!(registry.shells.len(), 1);

        assert!(remove_agent_shell(&path, "claude").unwrap());
        assert!(!agent_shell_exists(&path, "claude").unwrap());
        assert!(!remove_agent_shell(&path, "claude").unwrap());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_reserved_ids() {
        let path = std::env::temp_dir().join("mcpstore-agent-shell-reserved.json");
        assert!(ensure_agent_shell(&path, "all").is_err());
        assert!(ensure_agent_shell(&path, "store").is_err());
        assert!(ensure_agent_shell(&path, "  ").is_err());
    }
}
