use crate::perspective::tool_candidates::ToolCandidate;
use crate::perspective::tool_matching::{
    exact_match, fuzzy_match, no_prefix_match, prefix_match, suggestions,
};
use crate::{Result, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResolution {
    pub agent_id: String,
    pub local_service_name: String,
    pub global_service_name: String,
    pub local_tool_name: String,
    pub global_tool_name: String,
    pub canonical_tool_name: String,
    pub resolution_method: String,
    pub original_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableTool {
    pub name: String,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub service_name: Option<String>,
    #[serde(default)]
    pub global_service_name: Option<String>,
    #[serde(default)]
    pub global_tool_name: Option<String>,
}

impl AvailableTool {
    pub(in crate::perspective) fn display_name(&self) -> &str {
        self.name.as_str()
    }

    pub(in crate::perspective) fn canonical_name(&self) -> &str {
        self.original_name.as_deref().unwrap_or(self.name.as_str())
    }

    pub(in crate::perspective) fn service_name(&self) -> Result<&str> {
        self.service_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| StoreError::Other("available_tools 缺少 service_name".to_string()))
    }
}

pub fn resolve_tool(
    agent_id: &str,
    user_input: &str,
    available_tools: &[AvailableTool],
    target: &str,
    _strict: bool,
) -> Result<ToolResolution> {
    if target != "canonical" {
        return Err(StoreError::Other(
            "resolve_tool currently only supports target='canonical'".to_string(),
        ));
    }
    if agent_id.is_empty() {
        return Err(StoreError::Other("agent_id cannot be empty".to_string()));
    }
    if user_input.trim().is_empty() {
        return Err(StoreError::Other("Tool name cannot be empty".to_string()));
    }
    if available_tools.is_empty() {
        return Err(StoreError::Other(
            "available_tools cannot be empty, must provide tool list for resolution".to_string(),
        ));
    }

    let user_input = user_input.trim();
    let candidates = available_tools
        .iter()
        .map(ToolCandidate::from_tool)
        .collect::<Result<Vec<_>>>()?;

    if let Some(candidate) = exact_match(user_input, &candidates)? {
        return build_tool_resolution(agent_id, user_input, candidate, "exact_match");
    }
    if let Some(candidate) = prefix_match(user_input, &candidates)? {
        return build_tool_resolution(agent_id, user_input, candidate, "prefix_match");
    }
    if let Some(candidate) = no_prefix_match(user_input, &candidates)? {
        return build_tool_resolution(agent_id, user_input, candidate, "no_prefix_match");
    }
    if let Some(candidate) = fuzzy_match(user_input, &candidates)? {
        return build_tool_resolution(agent_id, user_input, candidate, "fuzzy_match");
    }

    let suggestions = suggestions(user_input, &candidates);
    if suggestions.is_empty() {
        Err(StoreError::Other(format!(
            "Tool '{user_input}' not found and no similar suggestions available"
        )))
    } else {
        Err(StoreError::Other(format!(
            "Tool '{user_input}' not found. Did you mean: {}?",
            suggestions.join(", ")
        )))
    }
}

fn build_tool_resolution(
    agent_id: &str,
    user_input: &str,
    candidate: ToolCandidate,
    resolution_method: &str,
) -> Result<ToolResolution> {
    Ok(ToolResolution {
        agent_id: agent_id.to_string(),
        local_service_name: candidate.service_name.clone(),
        global_service_name: candidate.global_service_name.clone(),
        local_tool_name: format!("{}_{}", candidate.service_name, candidate.original_name),
        global_tool_name: candidate.global_tool_name,
        canonical_tool_name: candidate.original_name,
        resolution_method: resolution_method.to_string(),
        original_input: user_input.to_string(),
    })
}
