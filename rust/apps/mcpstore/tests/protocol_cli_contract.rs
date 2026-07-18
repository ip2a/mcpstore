use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use mcpstore::{ScopeRef, ServiceInstanceKey};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcpstore"))
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mcpstore-protocol-cli-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

struct Fixture {
    dir: PathBuf,
    config: PathBuf,
    instance_id: String,
}

impl Fixture {
    fn new() -> Self {
        let dir = unique_temp_dir();
        let config = dir.join("mcp.json");
        let service_name = "protocol-fixture";
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mock_mcp_server.py")
            .canonicalize()
            .expect("failed to resolve fixture server");
        let pythonpath = format!(
            "{}:{}",
            repo_root().join("python/src").display(),
            repo_root()
                .join("rust/apps/mcpstore/tests/fixtures")
                .display()
        );
        std::fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "mcpServers": {
                    service_name: {
                        "command": "uv",
                        "args": [
                            "run",
                            "--project",
                            repo_root().join("python").display().to_string(),
                            "python",
                            fixture.display().to_string(),
                        ],
                        "env": {"PYTHONPATH": pythonpath},
                        "transport": "stdio",
                    }
                }
            }))
            .expect("failed to encode fixture config"),
        )
        .expect("failed to write fixture config");
        let instance_id = ServiceInstanceKey::new(service_name, ScopeRef::Store)
            .instance_id()
            .to_string();
        Self {
            dir,
            config,
            instance_id,
        }
    }

    fn command(&self, args: &[&str]) -> Output {
        let socket = self.dir.join("unused.sock");
        Command::new(cli_bin())
            .args(args)
            .arg("--config-path")
            .arg(&self.config)
            .env("MCPSTORE_SOCKET", socket)
            .current_dir(repo_root().join("rust"))
            .output()
            .expect("failed to run mcpstore CLI")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn parse_json_output(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: status={}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    serde_json::from_slice(&output.stdout).expect("stdout must be one JSON value")
}

#[test]
fn resource_list_and_read_have_machine_contracts() {
    let fixture = Fixture::new();

    let listed = parse_json_output(&fixture.command(&[
        "resource",
        "list",
        &fixture.instance_id,
        "--output",
        "json",
    ]));
    assert_eq!(listed["event"], "resource.listed");
    assert_eq!(listed["instance_id"], fixture.instance_id);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["resources"][0]["uri"], "fixture://docs/readme");

    let read = parse_json_output(&fixture.command(&[
        "resource",
        "read",
        &fixture.instance_id,
        "fixture://docs/readme",
        "--output",
        "json",
    ]));
    assert_eq!(read["event"], "resource.read");
    assert_eq!(read["uri"], "fixture://docs/readme");
    assert!(read["resource"].is_object());
}

#[test]
fn prompt_list_and_get_have_machine_contracts() {
    let fixture = Fixture::new();

    let listed = parse_json_output(&fixture.command(&[
        "prompt",
        "list",
        &fixture.instance_id,
        "--output",
        "json",
    ]));
    assert_eq!(listed["event"], "prompt.listed");
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["prompts"][0]["name"], "explain");

    let got = parse_json_output(&fixture.command(&[
        "prompt",
        "get",
        &fixture.instance_id,
        "explain",
        "--arguments",
        r#"{"topic":"Rust"}"#,
        "--output",
        "json",
    ]));
    assert_eq!(got["event"], "prompt.get");
    assert_eq!(got["prompt_name"], "explain");
    assert!(got["prompt"].is_object());
}

#[test]
fn completion_without_server_capability_has_stable_error() {
    let fixture = Fixture::new();
    let output = fixture.command(&[
        "complete",
        &fixture.instance_id,
        "--reference-kind",
        "prompt",
        "--reference",
        "explain",
        "--argument-name",
        "topic",
        "--value",
        "Ru",
        "--output",
        "jsonl",
    ]);

    assert_eq!(output.status.code(), Some(20));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid JSON error");
    assert_eq!(error["event"], "protocol.failed");
    assert_eq!(error["error"]["code"], "capability_unsupported");
    assert_eq!(error["instance_id"], fixture.instance_id);
}

#[test]
fn protocol_invalid_input_is_stable_and_does_not_pollute_stdout() {
    let fixture = Fixture::new();
    let output = fixture.command(&[
        "prompt",
        "get",
        &fixture.instance_id,
        "explain",
        "--arguments",
        "[]",
        "--output",
        "jsonl",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid JSON error");
    assert_eq!(error["event"], "protocol.failed");
    assert_eq!(error["error"]["code"], "invalid_input");
}

#[test]
fn invalid_instance_id_has_stable_error_without_connecting() {
    let fixture = Fixture::new();
    let output = fixture.command(&["resource", "list", "not-an-instance", "--output", "json"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid JSON error");
    assert_eq!(error["event"], "protocol.failed");
    assert_eq!(error["error"]["code"], "invalid_input");
    assert_eq!(error["instance_id"], Value::Null);
}
