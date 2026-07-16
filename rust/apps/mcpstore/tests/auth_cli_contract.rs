//! Process-level contract for the OAuth CLI error envelope.
//!
//! The typed classification correctness (refresh failed, callback rejected, auth
//! required, timeout, etc.) is covered by unit tests in `commands::auth`. This file
//! asserts the process-level machine contract that downstream scripts and CI rely on:
//!
//! - JSON errors are emitted on stderr as a single valid JSON Lines object.
//! - The object carries stable fields: `event`, `error.code`, `error.category`,
//!   `error.retryable`, `error.message`.
//! - The process exits non-zero.
//! - No secret material leaks into the output.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcpstore"))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mcpstore-auth-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

#[test]
fn auth_error_envelope_is_structured_non_sensitive_and_exits_nonzero() {
    let dir = unique_temp_dir("envelope");
    let config = dir.join("mcp.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({ "mcpServers": {} })).unwrap(),
    )
    .unwrap();

    // An unknown instance produces a deterministic, non-sensitive failure. The
    // envelope shape is the contract under test, not the specific category.
    let output = Command::new(cli_bin())
        .arg("auth")
        .arg("refresh")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--output")
        .arg("json")
        .arg("--config-path")
        .arg(&config)
        .output()
        .expect("failed to run mcpstore");

    assert!(
        !output.status.success(),
        "auth refresh on an unknown instance must exit non-zero"
    );

    // The JSON error is written to stderr as a single JSON Lines object (one line,
    // optionally terminated by a trailing newline as required by the JSON Lines spec).
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    assert!(
        trimmed.starts_with('{'),
        "expected a JSON object on stderr, got: {stderr}"
    );
    assert_eq!(
        trimmed.matches('\n').count(),
        0,
        "error envelope must be a single JSON Lines object, got: {stderr}"
    );

    let value: Value = serde_json::from_str(trimmed).expect("valid JSON error envelope");
    assert_eq!(value["event"], "error");
    let error = &value["error"];
    assert!(error["code"].is_string(), "missing stable error.code");
    assert!(
        error["category"].is_string(),
        "missing stable error.category"
    );
    assert!(
        error["retryable"].is_boolean(),
        "missing stable error.retryable"
    );
    assert!(error["message"].is_string(), "missing error.message");

    // Stable category/code vocabulary: must be one of the known machine values.
    let category = error["category"].as_str().unwrap();
    let known_categories = [
        "provider_unavailable",
        "authorization_start",
        "callback",
        "timeout",
        "refresh",
        "insufficient_scope",
        "auth_required",
        "secure_storage",
        "invalid_config",
        "unsupported",
        "missing_credential",
        "unknown",
    ];
    assert!(
        known_categories.contains(&category),
        "error.category {category:?} is not part of the stable vocabulary"
    );

    // Secrecy: no OAuth/PKCE material may appear anywhere in process output.
    let combined = format!("{}{}", stderr, String::from_utf8_lossy(&output.stdout));
    for forbidden in [
        "access_token",
        "refresh_token",
        "client_secret",
        "pkce_verifier",
        "authorization_code",
    ] {
        assert!(
            !combined.to_lowercase().contains(forbidden),
            "forbidden secret material {forbidden:?} leaked into output: {combined}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
