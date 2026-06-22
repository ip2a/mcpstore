use crate::perspective::generate_tool_global_name;
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
