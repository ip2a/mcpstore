use mcpstore_registry::ConnectionStatus;
use serde::Serialize;
use std::collections::HashMap;

pub(super) struct StatusMeta {
    pub class_name: &'static str,
    pub label: &'static str,
}

pub(super) fn status_meta(status: ConnectionStatus) -> StatusMeta {
    match status {
        ConnectionStatus::Connected => StatusMeta {
            class_name: "status-connected",
            label: "Connected",
        },
        ConnectionStatus::Disconnected => StatusMeta {
            class_name: "status-disconnected",
            label: "Disconnected",
        },
        ConnectionStatus::Connecting => StatusMeta {
            class_name: "status-connecting",
            label: "Connecting",
        },
        ConnectionStatus::Error => StatusMeta {
            class_name: "status-error",
            label: "Error",
        },
    }
}

pub(super) fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

pub(super) fn trim_optional(value: Option<&String>) -> Option<String> {
    value
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub(super) fn pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

pub(super) fn format_added_time(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".to_string();
    }
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

pub(super) fn url_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(super) fn parse_kv_lines(value: &str, label: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for (idx, raw_line) in value.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            return Err(format!(
                "{label} line {} must use KEY=VALUE format",
                idx + 1
            ));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("{label} line {} KEY cannot be empty", idx + 1));
        }
        out.insert(key.to_string(), val.trim().to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_handles_utf8() {
        assert_eq!(truncate_chars("Chinese service description", 4), "Chin...");
        assert_eq!(truncate_chars("Short text", 16), "Short text");
    }

    #[test]
    fn parse_kv_lines_rejects_invalid_rows() {
        let err = parse_kv_lines("TOKEN=abc\nINVALID", "env vars").unwrap_err();
        assert!(err.contains("line 2"));
    }

    #[test]
    fn parse_kv_lines_accepts_blank_rows() {
        let parsed = parse_kv_lines("A=1\n\nB = two", "env vars").unwrap();
        assert_eq!(parsed.get("A").map(String::as_str), Some("1"));
        assert_eq!(parsed.get("B").map(String::as_str), Some("two"));
    }
}
