use crate::{Result, StoreError};
use serde::{Deserialize, Serialize};

pub const GLOBAL_AGENT_STORE: &str = "global_agent_store";
pub const AGENT_SEPARATOR: &str = "_byagent_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentScopedName {
    pub agent_id: Option<String>,
    pub local_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceResolution {
    pub agent_id: String,
    pub local_name: String,
    pub global_name: String,
    pub resolution_method: String,
    pub original_input: String,
}

pub fn generate_service_global_name(original_name: &str, agent_id: &str) -> Result<String> {
    if original_name.is_empty() {
        return Err(StoreError::Other(
            "Service original name cannot be empty".to_string(),
        ));
    }
    if agent_id.is_empty() {
        return Err(StoreError::Other("Agent ID cannot be empty".to_string()));
    }
    if agent_id == GLOBAL_AGENT_STORE {
        Ok(original_name.to_string())
    } else {
        Ok(format!("{original_name}{AGENT_SEPARATOR}{agent_id}"))
    }
}

pub fn generate_tool_global_name(
    service_global_name: &str,
    tool_original_name: &str,
) -> Result<String> {
    if service_global_name.is_empty() {
        return Err(StoreError::Other(
            "Service global name cannot be empty".to_string(),
        ));
    }
    if tool_original_name.is_empty() {
        return Err(StoreError::Other(
            "Tool original name cannot be empty".to_string(),
        ));
    }
    let prefix = format!("{service_global_name}_");
    if tool_original_name.starts_with(&prefix) {
        Ok(tool_original_name.to_string())
    } else {
        Ok(format!("{service_global_name}_{tool_original_name}"))
    }
}

pub fn parse_agent_scoped(name: &str) -> Result<AgentScopedName> {
    if name.is_empty() {
        return Err(StoreError::Other(
            "Service name cannot be empty".to_string(),
        ));
    }

    if let Some((local_name, agent_id)) = name.rsplit_once(AGENT_SEPARATOR) {
        if local_name.is_empty() || agent_id.is_empty() {
            return Err(StoreError::Other(format!(
                "Invalid Agent service name format: {name}"
            )));
        }
        return Ok(AgentScopedName {
            agent_id: Some(agent_id.trim().to_string()),
            local_name: local_name.trim().to_string(),
        });
    }

    if let Some((agent_id, local_name)) = name.split_once(':') {
        if !agent_id.is_empty() && !local_name.is_empty() {
            return Ok(AgentScopedName {
                agent_id: Some(agent_id.to_string()),
                local_name: local_name.to_string(),
            });
        }
    }

    Ok(AgentScopedName {
        agent_id: None,
        local_name: name.to_string(),
    })
}

pub fn normalize_service_name(
    agent_id: &str,
    name: &str,
    target: &str,
    strict: bool,
) -> Result<ServiceResolution> {
    if target != "global" && target != "local" {
        return Err(StoreError::Other(
            "target must be 'global' or 'local'".to_string(),
        ));
    }
    if agent_id.is_empty() {
        return Err(StoreError::Other("agent_id cannot be empty".to_string()));
    }
    if name.is_empty() {
        return Err(StoreError::Other(
            "Service name cannot be empty".to_string(),
        ));
    }

    let parsed = parse_agent_scoped(name)?;
    if let Some(parsed_agent) = parsed.agent_id.as_deref() {
        if parsed_agent != agent_id {
            if agent_id == GLOBAL_AGENT_STORE {
                return Ok(ServiceResolution {
                    agent_id: agent_id.to_string(),
                    local_name: name.to_string(),
                    global_name: name.to_string(),
                    resolution_method: "global_agent_passthrough".to_string(),
                    original_input: name.to_string(),
                });
            }
            if strict {
                return Err(StoreError::Other(format!(
                    "Service belongs to agent_id={parsed_agent} which differs from target agent_id={agent_id}"
                )));
            }
        }
    }

    let resolution_method = if name.contains(AGENT_SEPARATOR) {
        "parsed_byagent"
    } else if parsed.agent_id.is_some() {
        "agent_prefix"
    } else {
        "assume_local"
    };
    let local_name = parsed.local_name;
    let global_name = generate_service_global_name(&local_name, agent_id)?;

    Ok(ServiceResolution {
        agent_id: agent_id.to_string(),
        local_name,
        global_name,
        resolution_method: resolution_method.to_string(),
        original_input: name.to_string(),
    })
}
