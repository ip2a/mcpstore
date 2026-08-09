//! Pure in-memory BM25 tool search. Zero external dependencies: the tokenizer
//! splits on non-alphanumeric runs via a `char` predicate, so no `regex` is
//! needed at this layer.
//!
//! See `tools.rs::build_search_tools` for the MCP tool surface that exposes
//! [`search_tools`] to agents as `mcpstore_search_tools`.

use rmcp::model::Tool;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// One search hit. `name` is what the agent feeds back into a normal tool call.
#[derive(Debug, Clone, Serialize)]
pub(super) struct ToolSearchHit {
    pub name: String,
    pub description: Option<String>,
    pub score: f64,
}

/// BM25-rank `tools` by `query`, returning up to `top_k` matches.
///
/// Document text for each tool = name (repeated for weight) + description +
/// the names and descriptions of every `input_schema` property. Results are
/// bounded by the caller's `tools` slice, so scope/session visibility isolation
/// is inherited from `McpStoreServer::current_tools` upstream.
pub(super) fn search_tools(tools: &[Tool], query: &str, top_k: usize) -> Vec<ToolSearchHit> {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;

    let docs: Vec<Vec<String>> = tools.iter().map(|t| tokenize(&doc_text(t))).collect();
    let n = docs.len();
    if n == 0 {
        return Vec::new();
    }
    let n_f = n as f64;
    let avgdl = (docs.iter().map(|d| d.len() as f64).sum::<f64>() / n_f).max(1.0);

    // Document frequency per unique term. No inverted index needed — df + avgdl
    // are all BM25 scoring requires.
    let mut df: HashMap<String, usize> = HashMap::new();
    for doc in &docs {
        for term in doc.iter().collect::<HashSet<&String>>() {
            *df.entry(term.clone()).or_default() += 1;
        }
    }

    let q_terms: HashSet<String> = tokenize(query).into_iter().collect();
    if q_terms.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<ToolSearchHit> = tools
        .iter()
        .enumerate()
        .map(|(i, tool)| {
            let doc = &docs[i];
            let dl = doc.len() as f64;
            let mut score = 0.0;
            for term in &q_terms {
                let freq = doc.iter().filter(|d| d.as_str() == term).count() as f64;
                if freq == 0.0 {
                    continue;
                }
                let df_val = *df.get(term).unwrap_or(&1) as f64;
                let idf = (((n_f - df_val + 0.5) / (df_val + 0.5)) + 1.0).ln();
                let denom = freq + K1 * (1.0 - B + B * dl / avgdl);
                score += idf * freq * (K1 + 1.0) / denom.max(1e-9);
            }
            ToolSearchHit {
                name: tool.name.to_string(),
                description: tool.description.as_deref().map(str::to_string),
                score,
            }
        })
        .collect();

    // Drop non-matches (score 0) so an agent asking for top_k never gets back
    // unrelated tools padded up to the bound.
    scored.retain(|hit| hit.score > 0.0);
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k.max(1));
    scored
}

/// Searchable text for one tool. The name is repeated so it dominates term
/// frequencies and name hits outrank description hits.
fn doc_text(tool: &Tool) -> String {
    let name = tool.name.as_ref();
    let mut text = format!("{name} {name} {name}");
    if let Some(desc) = tool.description.as_deref() {
        text.push(' ');
        text.push_str(desc);
    }
    if let Some(props) = tool
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (key, value) in props {
            text.push(' ');
            text.push_str(key);
            if let Some(desc) = value.get("description").and_then(Value::as_str) {
                text.push(' ');
                text.push_str(desc);
            }
        }
    }
    text
}

/// Lowercase and split on non-alphanumeric runs; drop tokens of length ≤ 1.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Tool;
    use serde_json::json;

    fn tool(name: &str, desc: &str) -> Tool {
        Tool::new(
            name.to_string(),
            desc.to_string(),
            std::sync::Arc::new(serde_json::Map::new()),
        )
    }

    fn tool_with_props(name: &str, desc: &str, props: Value) -> Tool {
        let schema: serde_json::Map<String, Value> =
            serde_json::from_value(json!({"type": "object", "properties": props})).unwrap();
        Tool::new(
            name.to_string(),
            desc.to_string(),
            std::sync::Arc::new(schema),
        )
    }

    #[test]
    fn ranks_name_matches_above_description_only_matches() {
        let tools = vec![
            tool("search_index", "rebuild the cache layer"),
            tool("rebuild_cache", "trigger a full refresh"),
            tool("unrelated", "does something else entirely"),
        ];
        let hits = search_tools(&tools, "rebuild cache", 3);
        // rebuild_cache carries both query terms in its name (×3 weight);
        // search_index carries them only in its description.
        assert!(!hits.is_empty());
        assert_eq!(hits[0].name, "rebuild_cache");
        // "unrelated" matches nothing → filtered out.
        assert!(hits.iter().all(|h| h.name != "unrelated"));
    }

    #[test]
    fn respects_top_k_bound() {
        let tools = vec![
            tool("alpha", "search me"),
            tool("beta", "search me"),
            tool("gamma", "search me"),
        ];
        let hits = search_tools(&tools, "search", 2);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn empty_query_returns_nothing() {
        let tools = vec![tool("alpha", "search me")];
        assert!(search_tools(&tools, "", 5).is_empty());
    }

    #[test]
    fn empty_tools_returns_nothing() {
        assert!(search_tools(&[], "anything", 5).is_empty());
    }

    #[test]
    fn tokenizes_input_schema_property_names_and_descriptions() {
        let tools = vec![
            tool_with_props(
                "data_tool",
                "generic helper",
                json!({"query_string": {"description": "the BM25 search phrase"}}),
            ),
            tool("other", "unrelated"),
        ];
        let hits = search_tools(&tools, "bm25", 2);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "data_tool");
    }
}
