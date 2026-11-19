//! Per-kind catalog search for one gateway mount. Pure layer only: entry
//! shapes, the activation truth table, BM25 ranking, the tokenizer, and the
//! result protocol. No store access, no I/O — the gateway owns sampling,
//! invalidation, and dispatch.
//!
//! Helpers activate per kind when the mount allows search and that kind's
//! visible catalog has at least [`SEARCH_ACTIVATION_THRESHOLD`] published
//! entries. Business bindings always win name collisions: a helper whose
//! name a downstream tool already owns stays suppressed.

use rmcp::model::{Prompt, Resource, ResourceTemplate, Tool};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// Published helper names. Deliberately unprefixed: they are part of the
/// downstream tool surface a business tool can collide with (business wins).
pub(super) const SEARCH_TOOLS_HELPER: &str = "search_tools";
pub(super) const SEARCH_PROMPT_HELPER: &str = "search_prompt";
pub(super) const SEARCH_RESOURCE_HELPER: &str = "search_resource";

/// Activation floor per kind. Ten-plus tools is Claude's tool-search
/// guidance; the prompt/resource thresholds are this project's choice, not an
/// MCP standard, and make no token-budget claim.
pub(super) const SEARCH_ACTIVATION_THRESHOLD: usize = 10;

/// Shared input contract: `limit` defaults to 5 (Claude's default), accepts
/// 1..=20.
pub(super) const SEARCH_DEFAULT_LIMIT: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum EntryKind {
    Tool,
    Prompt,
    Resource,
    ResourceTemplate,
}

impl EntryKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Prompt => "prompt",
            Self::Resource => "resource",
            Self::ResourceTemplate => "resource_template",
        }
    }
}

/// One searchable catalog row, already projected to its public identity.
#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub kind: EntryKind,
    /// Published name, uri or uriTemplate — what the caller feeds back into
    /// `tools/call`, `prompts/get` or `resources/read`.
    pub key: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Tokenized public metadata only (name/title/description, schema
    /// property names and descriptions, uri, mime). Never hidden canonical
    /// names, defaults, credentials, or content. The key is repeated so name
    /// hits outrank description hits under BM25.
    pub terms: Vec<String>,
}

/// One mount's discovered prompt/resource/template catalogs. `None` = kind
/// not (yet) completely observed; `Some([])` = observed empty. One instance
/// belongs to exactly one gateway mount — never shared across mounts or
/// client names. The fingerprints pair each column with the authority state
/// (membership + routes) it was captured under; a mismatch at read time
/// clears the column instead of serving stale rows.
#[derive(Default, Clone)]
pub(super) struct DiscoveryRows {
    /// Catalog generation the rows were published against; late responses
    /// from older generations are dropped.
    pub generation: u64,
    pub prompts: Option<Arc<[Entry]>>,
    pub resources: Option<Arc<[Entry]>>,
    pub templates: Option<Arc<[Entry]>>,
    pub prompt_fingerprint: u64,
    /// Covers both resource columns: resource catalog changes invalidate
    /// resource and template pages together.
    pub resource_fingerprint: u64,
}

/// Effective helper names for one published view, in fixed declaration
/// order. `tool_keys` are the visible BUSINESS bindings only — management
/// tools never enter the denominator. A helper whose name a business tool
/// owns stays off (the caller records the suppression as a projection issue).
pub(super) fn active_helper_names(
    enabled: bool,
    tool_keys: &BTreeSet<String>,
    rows: &DiscoveryRows,
) -> Vec<&'static str> {
    if !enabled {
        return Vec::new();
    }
    let mut names = Vec::new();
    if tool_keys.len() >= SEARCH_ACTIVATION_THRESHOLD && !tool_keys.contains(SEARCH_TOOLS_HELPER) {
        names.push(SEARCH_TOOLS_HELPER);
    }
    if let Some(prompts) = &rows.prompts {
        if prompts.len() >= SEARCH_ACTIVATION_THRESHOLD
            && !tool_keys.contains(SEARCH_PROMPT_HELPER)
        {
            names.push(SEARCH_PROMPT_HELPER);
        }
    }
    if let Some((entries, _, _)) = resource_view(rows) {
        if entries.len() >= SEARCH_ACTIVATION_THRESHOLD
            && !tool_keys.contains(SEARCH_RESOURCE_HELPER)
        {
            names.push(SEARCH_RESOURCE_HELPER);
        }
    }
    names
}

/// Resource + template entries deduped by `(kind, public key)`, with the
/// discovery state of each page. `None` while neither page has been observed
/// completely; known entries from one page still count while the other page
/// stays unknown.
pub(super) fn resource_view(rows: &DiscoveryRows) -> Option<(Vec<&Entry>, bool, bool)> {
    let resources = rows.resources.as_ref();
    let templates = rows.templates.as_ref();
    (resources.is_some() || templates.is_some()).then(|| {
        let mut entries: Vec<&Entry> = Vec::with_capacity(
            resources.map_or(0, |page| page.len()) + templates.map_or(0, |page| page.len()),
        );
        if let Some(resources) = resources {
            entries.extend(resources.iter());
        }
        if let Some(templates) = templates {
            entries.extend(templates.iter());
        }
        entries.sort_by(|left, right| {
            (left.kind, left.key.as_str()).cmp(&(right.kind, right.key.as_str()))
        });
        entries.dedup_by(|left, right| left.kind == right.kind && left.key == right.key);
        (entries, resources.is_some(), templates.is_some())
    })
}

/// Search entries for the current tool bindings. Tools are always completely
/// known from the mount snapshot, so this never returns unknown columns.
pub(super) fn entries_from_tools<'a>(tools: impl IntoIterator<Item = &'a Tool>) -> Vec<Entry> {
    tools
        .into_iter()
        .map(|tool| {
            let description = tool.description.as_deref().map(str::to_string);
            let mut terms = weighted_terms(tool.name.as_ref());
            if let Some(title) = &tool.title {
                terms.extend(tokenize(title));
            }
            if let Some(description) = &tool.description {
                terms.extend(tokenize(description));
            }
            for (name, schema) in tool
                .input_schema
                .get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                terms.extend(tokenize(name));
                if let Some(description) = schema.get("description").and_then(Value::as_str) {
                    terms.extend(tokenize(description));
                }
            }
            Entry {
                kind: EntryKind::Tool,
                key: tool.name.to_string(),
                title: tool.title.clone(),
                description,
                terms,
            }
        })
        .collect()
}

pub(super) fn entries_from_prompts<'a>(
    prompts: impl IntoIterator<Item = &'a Prompt>,
) -> Vec<Entry> {
    prompts
        .into_iter()
        .map(|prompt| {
            let mut terms = weighted_terms(&prompt.name);
            if let Some(title) = &prompt.title {
                terms.extend(tokenize(title));
            }
            if let Some(description) = &prompt.description {
                terms.extend(tokenize(description));
            }
            for argument in prompt.arguments.iter().flatten() {
                terms.extend(tokenize(&argument.name));
                if let Some(description) = &argument.description {
                    terms.extend(tokenize(description));
                }
            }
            Entry {
                kind: EntryKind::Prompt,
                key: prompt.name.clone(),
                title: prompt.title.clone(),
                description: prompt.description.clone(),
                terms,
            }
        })
        .collect()
}

pub(super) fn entries_from_resources<'a>(
    resources: impl IntoIterator<Item = &'a Resource>,
) -> Vec<Entry> {
    resources
        .into_iter()
        .map(|resource| {
            catalog_entry(
                EntryKind::Resource,
                &resource.uri,
                &resource.name,
                resource.title.as_ref(),
                resource.description.as_ref(),
                resource.mime_type.as_ref(),
            )
        })
        .collect()
}

pub(super) fn entries_from_templates<'a>(
    templates: impl IntoIterator<Item = &'a ResourceTemplate>,
) -> Vec<Entry> {
    templates
        .into_iter()
        .map(|template| {
            catalog_entry(
                EntryKind::ResourceTemplate,
                &template.uri_template,
                &template.name,
                template.title.as_ref(),
                template.description.as_ref(),
                template.mime_type.as_ref(),
            )
        })
        .collect()
}

/// Shared entry shape for resources and templates: the public address is the
/// identity (weighted), name/title/description/mime are searchable fields.
/// Tokenizing the template string itself already surfaces its visible
/// variable names (`{` and `}` split like any other separator).
fn catalog_entry(
    kind: EntryKind,
    key: &str,
    name: &str,
    title: Option<&String>,
    description: Option<&String>,
    mime_type: Option<&String>,
) -> Entry {
    let mut terms = weighted_terms(key);
    terms.extend(tokenize(name));
    if let Some(title) = title {
        terms.extend(tokenize(title));
    }
    if let Some(description) = description {
        terms.extend(tokenize(description));
    }
    if let Some(mime_type) = mime_type {
        terms.extend(tokenize(mime_type));
    }
    Entry {
        kind,
        key: key.to_string(),
        title: title.cloned(),
        description: description.cloned(),
        terms,
    }
}

/// Search terms for one entry's identity: the tokenized key repeated so key
/// hits dominate term frequencies.
fn weighted_terms(key: &str) -> Vec<String> {
    let mut terms = tokenize(key);
    let identity = terms.clone();
    terms.extend(identity.iter().cloned());
    terms.extend(identity);
    terms
}

/// BM25 rank over pre-tokenized entries (k1=1.5, b=0.75). Only entries with a
/// positive score return — an agent asking for `limit` never gets unrelated
/// rows padded up to the bound. Equal scores break by `(kind, key)` so the
/// order is stable across calls.
pub(super) fn rank<'a>(entries: &[&'a Entry], query: &str, limit: usize) -> Vec<&'a Entry> {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;

    let docs: Vec<&[String]> = entries.iter().map(|entry| entry.terms.as_slice()).collect();
    let n = docs.len();
    if n == 0 {
        return Vec::new();
    }
    let n_f = n as f64;
    let avgdl = (docs.iter().map(|doc| doc.len() as f64).sum::<f64>() / n_f).max(1.0);

    // Document frequency per unique term. No inverted index needed — df and
    // avgdl are all BM25 scoring requires at this catalog size.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for doc in &docs {
        for term in doc.iter().collect::<HashSet<&String>>() {
            *df.entry(term.as_str()).or_default() += 1;
        }
    }

    let q_terms: HashSet<String> = tokenize(query).into_iter().collect();
    if q_terms.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(f64, usize)> = docs
        .iter()
        .enumerate()
        .map(|(index, doc)| {
            let dl = doc.len() as f64;
            let mut score = 0.0;
            for term in &q_terms {
                let freq = doc
                    .iter()
                    .filter(|candidate| candidate.as_str() == term.as_str())
                    .count() as f64;
                if freq == 0.0 {
                    continue;
                }
                let df_val = df.get(term.as_str()).copied().unwrap_or(1) as f64;
                let idf = (((n_f - df_val + 0.5) / (df_val + 0.5)) + 1.0).ln();
                let denom = freq + K1 * (1.0 - B + B * dl / avgdl);
                score += idf * freq * (K1 + 1.0) / denom.max(1e-9);
            }
            (score, index)
        })
        .collect();
    scored.retain(|(score, _)| *score > 0.0);
    scored.sort_by(|(left_score, left_index), (right_score, right_index)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let left = &entries[*left_index];
                let right = &entries[*right_index];
                (left.kind, left.key.as_str()).cmp(&(right.kind, right.key.as_str()))
            })
    });
    scored.truncate(limit.max(1));
    scored
        .into_iter()
        .map(|(_, index)| entries[index])
        .collect()
}

/// Lowercase, then split runs by class: ASCII alphanumeric runs become
/// lowercase tokens (single-character runs dropped, matching the legacy
/// tokenizer); consecutive Han characters become adjacent bigrams; a lone Han
/// character is kept as-is. Han outside the common blocks falls back to
/// separator behavior. Only Chinese public metadata is promised to match —
/// no cross-language translation.
pub(super) fn tokenize(text: &str) -> Vec<String> {
    fn is_han(c: char) -> bool {
        // CJK Extension A, the Unified Ideographs block, compatibility
        // ideographs, and Extensions B through F.
        matches!(
            u32::from(c),
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF
        )
    }
    fn flush(run: &mut Vec<char>, run_is_ascii: bool, out: &mut Vec<String>) {
        if run_is_ascii {
            if run.len() > 1 {
                out.push(run.iter().collect());
            }
        } else if run.len() == 1 {
            out.push(run[0].to_string());
        } else {
            for pair in run.windows(2) {
                out.push(pair.iter().collect());
            }
        }
        run.clear();
    }

    let mut out = Vec::new();
    let mut run: Vec<char> = Vec::new();
    let mut run_is_ascii = false;
    for c in text.to_lowercase().chars() {
        let class = if c.is_ascii_alphanumeric() {
            0
        } else if is_han(c) {
            1
        } else {
            2
        };
        if class == 2 {
            if !run.is_empty() {
                flush(&mut run, run_is_ascii, &mut out);
            }
            continue;
        }
        let same_class = !run.is_empty() && run_is_ascii == (class == 0);
        if !same_class && !run.is_empty() {
            flush(&mut run, run_is_ascii, &mut out);
        }
        if run.is_empty() {
            run_is_ascii = class == 0;
        }
        run.push(c);
    }
    if !run.is_empty() {
        flush(&mut run, run_is_ascii, &mut out);
    }
    out
}

/// One parsed helper call: trimmed query plus the result bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SearchRequest {
    pub query: String,
    pub limit: usize,
}

/// Strict shared input: `{"query", "limit"}`. The query is required and must
/// be 1..=500 Unicode characters after trimming; limit is 1..=20, default 5.
/// Unknown fields are rejected so a fat-fingered `top_k` fails loudly
/// instead of being ignored.
pub(super) fn parse_search_arguments(arguments: &Map<String, Value>) -> Result<SearchRequest, String> {
    for field in arguments.keys() {
        if field != "query" && field != "limit" {
            return Err(format!("unexpected field: {field}"));
        }
    }
    let raw_query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "query must be a string".to_string())?
        .trim();
    let char_count = raw_query.chars().count();
    if char_count == 0 {
        return Err("query must not be empty".to_string());
    }
    if char_count > 500 {
        return Err("query must be at most 500 characters".to_string());
    }
    let limit = match arguments.get("limit") {
        None => SEARCH_DEFAULT_LIMIT,
        Some(value) => {
            let limit = value.as_u64().ok_or_else(|| {
                "limit must be an integer between 1 and 20".to_string()
            })?;
            if !(1..=20).contains(&limit) {
                return Err("limit must be an integer between 1 and 20".to_string());
            }
            limit as usize
        }
    };
    Ok(SearchRequest {
        query: raw_query.to_string(),
        limit,
    })
}

/// Structured result payload: items carry the address field for their kind
/// plus optional title and a description truncated to 240 characters;
/// `unknownKinds` lists the kinds this helper covers that were never
/// completely observed. Public keys are never truncated.
pub(super) fn search_payload(hits: &[&Entry], unknown_kinds: &[&str]) -> Value {
    let items: Vec<Value> = hits
        .iter()
        .map(|hit| {
            let mut item = serde_json::Map::new();
            item.insert("kind".into(), Value::String(hit.kind.as_str().into()));
            let address = match hit.kind {
                EntryKind::Tool => "name",
                EntryKind::Prompt => "name",
                EntryKind::Resource => "uri",
                EntryKind::ResourceTemplate => "uriTemplate",
            };
            item.insert(address.into(), Value::String(hit.key.clone()));
            if let Some(title) = &hit.title {
                item.insert("title".into(), Value::String(title.clone()));
            }
            if let Some(description) = &hit.description {
                let truncated: String = description.chars().take(240).collect();
                item.insert("description".into(), Value::String(truncated));
            }
            Value::Object(item)
        })
        .collect();
    serde_json::json!({
        "items": items,
        "unknownKinds": unknown_kinds,
    })
}

/// Human-readable companion text for clients that only surface content
/// blocks (rmcp attaches it alongside the structured payload).
pub(super) fn search_text(hits: &[&Entry], unknown_kinds: &[&str]) -> String {
    let mut lines = Vec::with_capacity(hits.len() + 1);
    match hits.len() {
        0 => lines.push("No matches.".to_string()),
        _ => lines.push(format!("{} match(es):", hits.len())),
    }
    for hit in hits {
        let mut line = format!("- [{}] {}", hit.kind.as_str(), hit.key);
        if let Some(description) = &hit.description {
            let truncated: String = description.chars().take(240).collect();
            line.push_str(&format!(" — {truncated}"));
        }
        lines.push(line);
    }
    if !unknown_kinds.is_empty() {
        lines.push(format!(
            "Not searched (never completely discovered): {}",
            unknown_kinds.join(", ")
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, description: &str) -> Tool {
        Tool::new(
            name.to_string(),
            description.to_string(),
            Arc::new(Map::new()),
        )
    }

    fn tool_with_props(name: &str, description: &str, props: Value) -> Tool {
        let schema: Map<String, Value> =
            serde_json::from_value(json!({"type": "object", "properties": props})).unwrap();
        Tool::new(name.to_string(), description.to_string(), Arc::new(schema))
    }

    fn keys(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn entries(count: usize, kind: EntryKind, key_prefix: &str) -> Option<Arc<[Entry]>> {
        Some(
            (0..count)
                .map(|index| Entry {
                    kind,
                    key: format!("{key_prefix}{index}"),
                    title: None,
                    description: Some(format!("{key_prefix} entry {index}")),
                    terms: tokenize(&format!("{key_prefix} entry {index}")),
                })
                .collect::<Arc<[Entry]>>(),
        )
    }

    fn rows(
        prompts: Option<Arc<[Entry]>>,
        resources: Option<Arc<[Entry]>>,
        templates: Option<Arc<[Entry]>>,
    ) -> DiscoveryRows {
        DiscoveryRows {
            prompts,
            resources,
            templates,
            ..DiscoveryRows::default()
        }
    }

    // ---- activation truth table (0 / 9 / 10) ----

    #[test]
    fn disabled_never_activates_any_helper() {
        let tool_keys: BTreeSet<String> = (0..12).map(|index| format!("t{index}")).collect();
        let full = rows(entries(10, EntryKind::Prompt, "p"), entries(10, EntryKind::Resource, "r"), entries(4, EntryKind::ResourceTemplate, "t"));
        assert!(active_helper_names(false, &tool_keys, &full).is_empty());
    }

    #[test]
    fn nine_tools_stay_below_the_threshold() {
        let tool_keys = keys(&["t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"]);
        assert!(active_helper_names(true, &tool_keys, &DiscoveryRows::default()).is_empty());
    }

    #[test]
    fn ten_tools_activate_only_the_tools_helper() {
        let tool_keys: BTreeSet<String> = (0..10).map(|index| format!("t{index}")).collect();
        assert_eq!(
            active_helper_names(true, &tool_keys, &DiscoveryRows::default()),
            [SEARCH_TOOLS_HELPER]
        );
    }

    #[test]
    fn ten_prompts_activate_only_the_prompt_helper() {
        let full = rows(entries(10, EntryKind::Prompt, "p"), None, None);
        assert_eq!(
            active_helper_names(true, &BTreeSet::new(), &full),
            [SEARCH_PROMPT_HELPER]
        );
    }

    #[test]
    fn helpers_activate_independently_per_kind() {
        // (tools, prompts, resources, templates) = (9, 10, 6, 4): the doc's
        // acceptance vector — search_prompt and search_resource appear,
        // search_tools does not.
        let tool_keys: BTreeSet<String> = (0..9).map(|index| format!("t{index}")).collect();
        let full = rows(
            entries(10, EntryKind::Prompt, "p"),
            entries(6, EntryKind::Resource, "r"),
            entries(4, EntryKind::ResourceTemplate, "t"),
        );
        assert_eq!(
            active_helper_names(true, &tool_keys, &full),
            [SEARCH_PROMPT_HELPER, SEARCH_RESOURCE_HELPER]
        );
    }

    #[test]
    fn resource_and_template_pages_sum_after_dedup() {
        // 6 resources + 4 templates = 10 → active; each page alone is not.
        let both = rows(
            None,
            entries(6, EntryKind::Resource, "r"),
            entries(4, EntryKind::ResourceTemplate, "t"),
        );
        assert_eq!(
            active_helper_names(true, &BTreeSet::new(), &both),
            [SEARCH_RESOURCE_HELPER]
        );
        let either = rows(
            None,
            entries(6, EntryKind::Resource, "r"),
            entries(4, EntryKind::ResourceTemplate, "t"),
        );
        let (view_entries, resources_known, templates_known) = resource_view(&either).unwrap();
        assert_eq!(view_entries.len(), 10);
        assert!(resources_known && templates_known);

        let only_resources = rows(None, entries(10, EntryKind::Resource, "r"), None);
        let (view, resources_known, templates_known) = resource_view(&only_resources).unwrap();
        assert_eq!(view.len(), 10);
        assert!(resources_known && !templates_known);
        assert_eq!(
            active_helper_names(true, &BTreeSet::new(), &only_resources),
            [SEARCH_RESOURCE_HELPER],
            "10 known resources with templates undiscovered still activates, marked incomplete"
        );

        let neither = rows(None, None, entries(10, EntryKind::ResourceTemplate, "t"));
        let (view, resources_known, templates_known) = resource_view(&neither).unwrap();
        assert_eq!(view.len(), 10);
        assert!(!resources_known && templates_known);
        assert_eq!(
            active_helper_names(true, &BTreeSet::new(), &neither),
            [SEARCH_RESOURCE_HELPER]
        );
    }

    #[test]
    fn unknown_columns_never_activate_their_helper() {
        let cold = DiscoveryRows::default();
        assert!(active_helper_names(true, &BTreeSet::new(), &cold).is_empty());
        let short = rows(entries(9, EntryKind::Prompt, "p"), None, None);
        assert!(active_helper_names(true, &BTreeSet::new(), &short).is_empty());
    }

    #[test]
    fn business_collision_suppresses_only_that_helper() {
        // 10 business keys, one of them named "search_prompt": that helper is
        // suppressed while the other two keep working.
        let tool_keys = keys(&["search_prompt", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"]);
        let full = rows(entries(10, EntryKind::Prompt, "p"), entries(6, EntryKind::Resource, "r"), entries(4, EntryKind::ResourceTemplate, "t"));
        assert_eq!(
            active_helper_names(true, &tool_keys, &full),
            [SEARCH_TOOLS_HELPER, SEARCH_RESOURCE_HELPER],
        );
    }

    // ---- ranking ----

    #[test]
    fn ranks_name_matches_above_description_only_matches() {
        let tools = entries_from_tools([
            &tool("search_index", "rebuild the cache layer"),
            &tool("rebuild_cache", "trigger a full refresh"),
            &tool("unrelated", "does something else entirely"),
        ]);
        let refs: Vec<&Entry> = tools.iter().collect();
        let hits = rank(&refs, "rebuild cache", 3);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].key, "rebuild_cache");
        assert!(hits.iter().all(|hit| hit.key != "unrelated"));
    }

    #[test]
    fn parameter_names_and_descriptions_are_searchable() {
        let tools = entries_from_tools([&tool_with_props(
            "data_tool",
            "generic helper",
            json!({"query_string": {"description": "the BM25 search phrase"}}),
        )]);
        let refs: Vec<&Entry> = tools.iter().collect();
        let hits = rank(&refs, "bm25", 2);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "data_tool");
    }

    #[test]
    fn chinese_metadata_matches_via_bigrams() {
        let prompts: Vec<Prompt> = vec![
            serde_json::from_value(json!({"name": "release_notes", "description": "生成每周发布规范"})).unwrap(),
            serde_json::from_value(json!({"name": "db_check", "description": "检查数据库迁移状态"})).unwrap(),
        ];
        let entries = entries_from_prompts(&prompts);
        let refs: Vec<&Entry> = entries.iter().collect();
        let hits = rank(&refs, "发布", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "release_notes");
        let hits = rank(&refs, "数据库", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "db_check");
        let hits = rank(&refs, "迁移", 5);
        assert_eq!(hits[0].key, "db_check");
    }

    #[test]
    fn equal_scores_break_by_kind_then_key() {
        let terms = tokenize("same words here");
        let make = |kind: EntryKind, key: &str| Entry {
            kind,
            key: key.to_string(),
            title: None,
            description: Some("same words here".to_string()),
            terms: terms.clone(),
        };
        let entries = vec![
            make(EntryKind::Prompt, "zzz"),
            make(EntryKind::Tool, "aaa"),
        ];
        let refs: Vec<&Entry> = entries.iter().collect();
        let hits = rank(&refs, "same words", 5);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].key, "aaa", "tool < prompt on ties");
        assert_eq!(hits[1].key, "zzz");
    }

    #[test]
    fn respects_limit_and_never_pads_with_unrelated_entries() {
        let tools = entries_from_tools([
            &tool("alpha", "search me"),
            &tool("beta", "search me"),
            &tool("gamma", "search me"),
        ]);
        let refs: Vec<&Entry> = tools.iter().collect();
        assert_eq!(rank(&refs, "search", 2).len(), 2);
        let hits = rank(&refs, "zzzznothing", 5);
        assert!(hits.is_empty());
    }

    #[test]
    fn empty_query_and_empty_entries_return_nothing() {
        let tools = entries_from_tools([&tool("alpha", "search me")]);
        let refs: Vec<&Entry> = tools.iter().collect();
        assert!(rank(&refs, "", 5).is_empty());
        assert!(rank(&[], "anything", 5).is_empty());
    }

    // ---- tokenizer ----

    #[test]
    fn tokenizer_handles_ascii_cjk_and_mixed_runs() {
        assert_eq!(
            tokenize("Release-Notes v2"),
            vec!["release".to_string(), "notes".to_string(), "v2".to_string()]
        );
        assert_eq!(tokenize("发布 release"), vec!["发布".to_string(), "release".to_string()]);
        assert_eq!(
            tokenize("abc发布def"),
            vec!["abc".to_string(), "发布".to_string(), "def".to_string()]
        );
        assert_eq!(tokenize("数据库"), vec!["数据".to_string(), "据库".to_string()]);
        assert_eq!(tokenize("迁"), vec!["迁".to_string()]);
        assert_eq!(tokenize("a b"), Vec::<String>::new(), "single ASCII runs are dropped");
    }

    #[test]
    fn tool_entries_weight_the_key_over_the_description() {
        let entries = entries_from_tools([&tool("greet", "say hello")]);
        let greet_count = entries[0]
            .terms
            .iter()
            .filter(|term| term.as_str() == "greet")
            .count();
        assert!(greet_count >= 3, "the key must be weighted");
    }

    // ---- input validation ----

    #[test]
    fn parses_the_shared_input_contract() {
        let mut args = Map::new();
        args.insert("query".into(), json!("  release notes  "));
        assert_eq!(
            parse_search_arguments(&args).unwrap(),
            SearchRequest { query: "release notes".into(), limit: 5 }
        );
        args.insert("limit".into(), json!(20));
        assert_eq!(parse_search_arguments(&args).unwrap().limit, 20);

        args.insert("limit".into(), json!(0));
        assert!(parse_search_arguments(&args).is_err());
        args.insert("limit".into(), json!(21));
        assert!(parse_search_arguments(&args).is_err());
        args.insert("limit".into(), json!("5"));
        assert!(parse_search_arguments(&args).is_err(), "string limit rejected");

        let mut missing = Map::new();
        assert!(parse_search_arguments(&missing).is_err());
        missing.insert("query".into(), json!("   "));
        assert!(parse_search_arguments(&missing).is_err());
        missing.insert("query".into(), json!("x".repeat(501)));
        assert!(parse_search_arguments(&missing).is_err());
        missing.insert("query".into(), json!("x".repeat(500)));
        assert!(parse_search_arguments(&missing).is_ok());

        let mut extra = Map::new();
        extra.insert("query".into(), json!("ok"));
        extra.insert("top_k".into(), json!(5));
        assert!(
            parse_search_arguments(&extra).is_err(),
            "the retired top_k parameter must be rejected, not ignored"
        );
    }

    // ---- result protocol ----

    #[test]
    fn payload_carries_address_fields_and_truncated_descriptions() {
        let entry = Entry {
            kind: EntryKind::Resource,
            key: "mcpstore://gateway/docs/release".to_string(),
            title: Some("发布规范".to_string()),
            description: Some("d".repeat(300)),
            terms: Vec::new(),
        };
        let payload = search_payload(&[&entry], &["resource_template"]);
        assert_eq!(payload["items"][0]["kind"], "resource");
        assert_eq!(payload["items"][0]["uri"], "mcpstore://gateway/docs/release");
        assert_eq!(payload["items"][0]["title"], "发布规范");
        assert_eq!(
            payload["items"][0]["description"].as_str().unwrap().chars().count(),
            240
        );
        assert_eq!(payload["unknownKinds"], json!(["resource_template"]));

        let template = Entry {
            kind: EntryKind::ResourceTemplate,
            key: "fixture://docs/{name}".to_string(),
            title: None,
            description: None,
            terms: Vec::new(),
        };
        let payload = search_payload(&[&template], &[]);
        assert_eq!(payload["items"][0]["uriTemplate"], "fixture://docs/{name}");
        assert!(payload["items"][0].get("name").is_none());

        let tool_entry = Entry {
            kind: EntryKind::Tool,
            key: "demo__greet".to_string(),
            title: None,
            description: None,
            terms: Vec::new(),
        };
        let payload = search_payload(&[&tool_entry], &[]);
        assert_eq!(payload["items"][0]["name"], "demo__greet");
        assert!(payload["items"][0].get("uri").is_none());
    }

    #[test]
    fn text_companion_lists_hits_and_unknown_kinds() {
        let entry = Entry {
            kind: EntryKind::Prompt,
            key: "release_notes".to_string(),
            title: None,
            description: Some("weekly notes".to_string()),
            terms: Vec::new(),
        };
        let text = search_text(&[&entry], &["resource"]);
        assert!(text.contains("[prompt] release_notes"));
        assert!(text.contains("weekly notes"));
        assert!(text.contains("resource"));
        assert!(search_text(&[], &[]).contains("No matches"));
    }
}
