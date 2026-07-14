use crate::perspective::tool_candidates::ToolCandidate;
use crate::{Result, StoreError};

pub(super) fn exact_match(
    user_input: &str,
    candidates: &[ToolCandidate],
) -> Result<Option<ToolCandidate>> {
    let matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.display_name == user_input || candidate.tool_name == user_input
        })
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "exact")
}

pub(super) fn prefix_match(
    user_input: &str,
    candidates: &[ToolCandidate],
) -> Result<Option<ToolCandidate>> {
    let matches = candidates
        .iter()
        .filter(|candidate| {
            let prefix = format!("{}_", candidate.service_name);
            user_input
                .strip_prefix(&prefix)
                .map(|suffix| suffix == candidate.tool_name || suffix == candidate.display_name)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    one_or_ambiguous(user_input, matches, "prefix")
}

pub(super) fn fuzzy_match(
    user_input: &str,
    candidates: &[ToolCandidate],
) -> Result<Option<ToolCandidate>> {
    let cleaned = clean_for_fuzzy(user_input);
    let matches = candidates
        .iter()
        .filter(|candidate| {
            is_fuzzy_match(&cleaned, &candidate.display_name)
                || is_fuzzy_match(&cleaned, &candidate.tool_name)
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

pub(super) fn suggestions(user_input: &str, candidates: &[ToolCandidate]) -> Vec<String> {
    let user_clean = clean_for_fuzzy(user_input);
    let mut scored = candidates
        .iter()
        .filter_map(|candidate| {
            let score =
                similarity_score(&user_clean, &candidate.display_name, &candidate.tool_name);
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
