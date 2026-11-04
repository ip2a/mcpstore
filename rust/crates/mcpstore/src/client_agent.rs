//! Client-to-agent binding: tracks which client owns which agent scope.
//!
//! When a client is wired, we assign it an agent_id (defaulting to the client's own id).
//! This mapping is persisted so that the agent scope remains stable across daemon restarts.
//! The agent itself doesn't need explicit registration — its first service instance creates it.

use crate::{Error, FailureCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAgentBinding {
    pub agent_id: String,
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientAgentRegistry {
    #[serde(default)]
    pub bindings: HashMap<String, ClientAgentBinding>,
}

pub fn load_client_agent_registry(path: &Path) -> Result<ClientAgentRegistry> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to parse {}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ClientAgentRegistry::default())
        }
        Err(error) => Err(Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", path.display()),
        )),
    }
}

pub fn save_client_agent_registry(path: &Path, registry: &ClientAgentRegistry) -> Result<()> {
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

/// Get or assign an agent_id for a client. If the client has no binding yet, creates one
/// with agent_id = client_id and persists it.
pub fn ensure_client_agent_binding(registry_path: &Path, client_id: &str) -> Result<String> {
    let mut registry = load_client_agent_registry(registry_path)?;

    if let Some(binding) = registry.bindings.get(client_id) {
        return Ok(binding.agent_id.clone());
    }

    let agent_id = client_id.to_string();
    let binding = ClientAgentBinding {
        agent_id: agent_id.clone(),
        created_at_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    registry.bindings.insert(client_id.to_string(), binding);
    save_client_agent_registry(registry_path, &registry)?;
    Ok(agent_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_binding_creates_and_persists_on_first_call() {
        let path = std::env::temp_dir().join(format!("mcpstore-agent-{}", std::process::id()));
        let agent_id = ensure_client_agent_binding(&path, "claude").unwrap();
        assert_eq!(agent_id, "claude");

        let registry = load_client_agent_registry(&path).unwrap();
        assert_eq!(registry.bindings.len(), 1);
        assert_eq!(registry.bindings["claude"].agent_id, "claude");

        let again = ensure_client_agent_binding(&path, "claude").unwrap();
        assert_eq!(again, "claude");
        let _ = fs::remove_file(path);
    }
}
