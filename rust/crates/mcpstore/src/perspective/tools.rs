use crate::perspective::tool_candidates::ToolCandidate;
use crate::perspective::tool_matching::{exact_match, fuzzy_match, prefix_match, suggestions};
use crate::{InstanceId, Result, ScopeRef, ServiceInstanceKey, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResolution {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    pub resolution_method: String,
    pub original_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableTool {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    pub name: String,
}

impl AvailableTool {
    pub(in crate::perspective) fn display_name(&self) -> &str {
        self.name.as_str()
    }

    pub(in crate::perspective) fn service_name(&self) -> Result<&str> {
        non_empty(&self.service_name, "service_name")
    }

    pub(in crate::perspective) fn tool_name(&self) -> Result<&str> {
        non_empty(&self.tool_name, "tool_name")
    }

    pub(in crate::perspective) fn validate(&self) -> Result<()> {
        self.service_name()?;
        self.tool_name()?;
        non_empty(&self.name, "name")?;
        let expected =
            ServiceInstanceKey::new(self.service_name.clone(), self.scope.clone()).instance_id();
        if self.instance_id != expected {
            return Err(StoreError::Other(format!(
                "available_tools instance_id {} does not match service_name '{}' and scope",
                self.instance_id, self.service_name
            )));
        }
        Ok(())
    }
}

pub fn resolve_tool(user_input: &str, available_tools: &[AvailableTool]) -> Result<ToolResolution> {
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
        return Ok(build_tool_resolution(user_input, candidate, "exact_match"));
    }
    if let Some(candidate) = prefix_match(user_input, &candidates)? {
        return Ok(build_tool_resolution(user_input, candidate, "prefix_match"));
    }
    if let Some(candidate) = fuzzy_match(user_input, &candidates)? {
        return Ok(build_tool_resolution(user_input, candidate, "fuzzy_match"));
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
    user_input: &str,
    candidate: ToolCandidate,
    resolution_method: &str,
) -> ToolResolution {
    ToolResolution {
        instance_id: candidate.instance_id,
        service_name: candidate.service_name,
        scope: candidate.scope,
        tool_name: candidate.tool_name,
        resolution_method: resolution_method.to_string(),
        original_input: user_input.to_string(),
    }
}

fn non_empty<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    if value.trim().is_empty() {
        Err(StoreError::Other(format!(
            "available_tools contains empty {field}"
        )))
    } else {
        Ok(value)
    }
}
