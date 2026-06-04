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
    fn display_name(&self) -> &str {
        self.name.as_str()
    }

    fn canonical_name(&self) -> &str {
        self.original_name.as_deref().unwrap_or(self.name.as_str())
    }

    fn service_name(&self) -> Result<&str> {
        self.service_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| StoreError::Other("available_tools 缺少 service_name".to_string()))
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCandidate {
    service_name: String,
    original_name: String,
    display_name: String,
    global_service_name: String,
    global_tool_name: String,
}

impl ToolCandidate {
    fn from_tool(tool: &AvailableTool) -> Result<Self> {
        let service_name = tool.service_name()?.to_string();
        let original_name = tool.canonical_name().to_string();
        let display_name = tool.display_name().to_string();
        let global_service_name = tool
            .global_service_name
            .clone()
            .unwrap_or_else(|| service_name.clone());
        let global_tool_name = match tool.global_tool_name.clone() {
            Some(name) => name,
            None => generate_tool_global_name(&global_service_name, &original_name)?,
        };
        Ok(Self {
            service_name,
            original_name,
            display_name,
            global_service_name,
            global_tool_name,
        })
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

fn exact_match(user_input: &str, candidates: &[ToolCandidate]) -> Result<Option<ToolCandidate>> {
    let matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.display_name == user_input || candidate.original_name == user_input
        })
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "exact")
}

fn prefix_match(user_input: &str, candidates: &[ToolCandidate]) -> Result<Option<ToolCandidate>> {
    let matches = candidates
        .iter()
        .filter(|candidate| {
            let prefix = format!("{}_", candidate.service_name);
            user_input
                .strip_prefix(&prefix)
                .map(|suffix| suffix == candidate.original_name || suffix == candidate.display_name)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "prefix")
}

fn no_prefix_match(
    user_input: &str,
    candidates: &[ToolCandidate],
) -> Result<Option<ToolCandidate>> {
    let matches = candidates
        .iter()
        .filter(|candidate| candidate.original_name == user_input)
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "no-prefix")
}

fn fuzzy_match(user_input: &str, candidates: &[ToolCandidate]) -> Result<Option<ToolCandidate>> {
    let cleaned = clean_for_fuzzy(user_input);
    let matches = candidates
        .iter()
        .filter(|candidate| {
            is_fuzzy_match(&cleaned, &candidate.display_name)
                || is_fuzzy_match(&cleaned, &candidate.original_name)
        })
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "fuzzy")
}

fn one_or_ambiguous(
    user_input: &str,
    matches: Vec<ToolCandidate>,
    method: &str,
) -> Result<Option<ToolCandidate>> {
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.into_iter().next()),
        count => Err(StoreError::Other(format!(
            "Ambiguous {method} tool name '{user_input}', matched {count} tools"
        ))),
    }
}

fn clean_for_fuzzy(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_fuzzy_match(user_clean: &str, target: &str) -> bool {
    let target_clean = clean_for_fuzzy(target);
    if user_clean.is_empty() || target_clean.is_empty() {
        return false;
    }
    user_clean.contains(&target_clean)
        || target_clean.contains(user_clean)
        || (user_clean.len() >= 3
            && (target_clean.starts_with(user_clean) || user_clean.starts_with(&target_clean)))
}

fn suggestions(user_input: &str, candidates: &[ToolCandidate]) -> Vec<String> {
    let user_clean = clean_for_fuzzy(user_input);
    let mut scored = candidates
        .iter()
        .filter_map(|candidate| {
            let score = similarity_score(
                &user_clean,
                &candidate.display_name,
                &candidate.original_name,
            );
            if score > 0.3 {
                Some((score, candidate.display_name.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(5).map(|(_, name)| name).collect()
}

fn similarity_score(user_clean: &str, display_name: &str, original_name: &str) -> f64 {
    [display_name, original_name]
        .into_iter()
        .map(|value| {
            let clean = clean_for_fuzzy(value);
            if user_clean == clean {
                1.0
            } else if clean.contains(user_clean) {
                0.8
            } else if clean.starts_with(user_clean) || user_clean.starts_with(&clean) {
                0.6
            } else {
                0.0
            }
        })
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<AvailableTool> {
        vec![
            AvailableTool {
                name: "weather_get_current_weather".to_string(),
                original_name: Some("get_current_weather".to_string()),
                service_name: Some("weather".to_string()),
                global_service_name: Some("weather_byagent_agent1".to_string()),
                global_tool_name: None,
            },
            AvailableTool {
                name: "docs_get_page".to_string(),
                original_name: Some("get_page".to_string()),
                service_name: Some("docs".to_string()),
                global_service_name: Some("docs_byagent_agent1".to_string()),
                global_tool_name: None,
            },
        ]
    }

    #[test]
    fn normalizes_agent_service_names() {
        let parsed = parse_agent_scoped("agent1:weather").unwrap();
        assert_eq!(parsed.agent_id.as_deref(), Some("agent1"));
        assert_eq!(parsed.local_name, "weather");

        let parsed = parse_agent_scoped("weather_byagent_shadow_byagent_agent1").unwrap();
        assert_eq!(parsed.agent_id.as_deref(), Some("agent1"));
        assert_eq!(parsed.local_name, "weather_byagent_shadow");

        let normalized = normalize_service_name("agent1", "weather", "global", true).unwrap();
        assert_eq!(normalized.global_name, "weather_byagent_agent1");
        assert_eq!(normalized.local_name, "weather");
    }

    #[test]
    fn resolves_prefixed_tool_to_canonical() {
        let resolved = resolve_tool(
            "agent1",
            "weather_get_current_weather",
            &tools(),
            "canonical",
            true,
        )
        .unwrap();
        assert_eq!(resolved.canonical_tool_name, "get_current_weather");
        assert_eq!(
            resolved.global_tool_name,
            "weather_byagent_agent1_get_current_weather"
        );
    }

    #[test]
    fn rejects_ambiguous_no_prefix_tool() {
        let mut tools = tools();
        tools.push(AvailableTool {
            name: "another_get_current_weather".to_string(),
            original_name: Some("get_current_weather".to_string()),
            service_name: Some("another".to_string()),
            global_service_name: None,
            global_tool_name: None,
        });
        let err = resolve_tool("agent1", "get_current_weather", &tools, "canonical", true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Ambiguous"));
    }
}
