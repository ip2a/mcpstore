use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mcpstore::{ScopeRef, ServiceInstanceKey};
use serde_json::{json, Value};

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcpstore"))
}

fn fixture_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/execution_mcp_server.py")
        .canonicalize()
        .expect("failed to resolve execution fixture")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mcpstore-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

struct Fixture {
    dir: PathBuf,
    config: PathBuf,
    marker: PathBuf,
    socket: PathBuf,
    instance_id: String,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let dir = unique_temp_dir(label);
        let config = dir.join("mcp.json");
        let marker = dir.join("marker.log");
        let socket = dir.join("unused.sock");
        let service_name = format!("execution-{label}");
        let fixture = fixture_script();
        std::fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "mcpServers": {
                    service_name.clone(): {
                        "command": "python3",
                        "args": [fixture],
                        "env": {
                            "MCP_EXECUTION_MARKER": marker,
                        },
                        "transport": "stdio",
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let instance_id = ServiceInstanceKey::new(service_name, ScopeRef::Store)
            .instance_id()
            .to_string();
        Self {
            dir,
            config,
            marker,
            socket,
            instance_id,
        }
    }

    fn command(&self, tool: &str) -> Command {
        self.command_with_output(tool, "jsonl")
    }

    fn command_with_output(&self, tool: &str, output: &str) -> Command {
        self.command_with_output_and_interactive(tool, output, false)
    }

    fn command_with_output_and_interactive(
        &self,
        tool: &str,
        output: &str,
        interactive_flag: bool,
    ) -> Command {
        let mut command = Command::new(cli_bin());
        command
            .args([
                "call",
                &self.instance_id,
                tool,
                "--output",
                output,
                "--config-path",
            ])
            .arg(&self.config)
            .env("MCPSTORE_SOCKET", &self.socket);
        if !interactive_flag {
            command.arg("--non-interactive");
        }
        command
    }

    fn input_file(&self, label: &str, value: &Value) -> PathBuf {
        let path = self.dir.join(format!("{label}.json"));
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    fn task_command(&self, tool: &str, output: &str) -> Command {
        let mut command = Command::new(cli_bin());
        command
            .args([
                "task",
                "run",
                &self.instance_id,
                tool,
                "--input",
                "{}",
                "--output",
                output,
                "--non-interactive",
                "--config-path",
            ])
            .arg(&self.config)
            .env("MCPSTORE_SOCKET", &self.socket);
        command
    }

    fn marker_text(&self) -> String {
        std::fs::read_to_string(&self.marker).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn json_lines(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}")))
        .collect()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn call_jsonl_streams_progress_and_completion() {
    let fixture = Fixture::new("progress");
    let output = fixture
        .command("progress")
        .output()
        .expect("failed to run progress call");
    assert_success(&output);
    assert!(
        output.stderr.is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = json_lines(&output.stdout);
    assert_eq!(events.len(), 4);
    assert_eq!(events[0]["event"], "execution.started");
    assert_eq!(events[1]["event"], "execution.progress");
    assert_eq!(events[2]["event"], "execution.progress");
    assert_eq!(events[3]["event"], "execution.completed");
    for event in &events {
        assert_eq!(event["instance_id"], fixture.instance_id);
        assert_eq!(event["tool_name"], "progress");
    }
    assert_eq!(
        events[3]["result"]["content"][0]["text"],
        "fixture-complete"
    );
}

#[test]
fn call_json_emits_one_terminal_object_without_progress() {
    let fixture = Fixture::new("json");
    let output = fixture
        .command_with_output("progress", "json")
        .output()
        .expect("failed to run JSON progress call");
    assert_success(&output);
    assert!(output.stderr.is_empty());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["event"], "execution.completed");
    assert_eq!(value["instance_id"], fixture.instance_id);
    assert_eq!(value["tool_name"], "progress");
    assert_eq!(value["result"]["content"][0]["text"], "fixture-complete");
}

#[test]
fn call_tool_error_has_stable_exit_and_machine_error() {
    let fixture = Fixture::new("tool-error");
    let output = fixture
        .command("tool_error")
        .output()
        .expect("failed to run tool error call");

    assert_eq!(output.status.code(), Some(33));
    let events = json_lines(&output.stdout);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "execution.started");
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["event"], "execution.failed");
    assert_eq!(error["error"]["code"], "tool_failed");
    assert_eq!(error["instance_id"], fixture.instance_id);
    assert_eq!(error["tool_name"], "tool_error");
}

#[test]
fn call_idle_timeout_has_stable_exit_and_jsonl_error() {
    let fixture = Fixture::new("timeout");
    let output = fixture
        .command("hang")
        .args(["--timeout", "1"])
        .output()
        .expect("failed to run timeout call");

    assert_eq!(output.status.code(), Some(31));
    let events = json_lines(&output.stdout);
    assert_eq!(events[0]["event"], "execution.started");
    let error: Value = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(error["event"], "execution.timed_out");
    assert_eq!(error["error"]["code"], "execution_timed_out");
    assert_eq!(error["instance_id"], fixture.instance_id);
    assert!(fixture.marker_text().contains("cancelled:"));
}

#[test]
fn call_max_total_timeout_is_not_reset_by_progress() {
    let fixture = Fixture::new("max-total-timeout");
    let output = fixture
        .command("keep_progress")
        .args(["--timeout", "2", "--max-total-timeout", "1"])
        .output()
        .expect("failed to run max-total-timeout call");

    assert_eq!(output.status.code(), Some(31));
    let events = json_lines(&output.stdout);
    assert_eq!(events[0]["event"], "execution.started");
    assert!(events
        .iter()
        .any(|event| event["event"] == "execution.progress"));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["event"], "execution.timed_out");
    assert_eq!(error["error"]["code"], "execution_timed_out");
    assert!(fixture.marker_text().contains("cancelled:"));
}

#[cfg(unix)]
#[test]
fn call_ctrl_c_sends_typed_cancellation_and_exits_stably() {
    let fixture = Fixture::new("cancel");
    let child = fixture
        .command("hang")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cancellable call");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !fixture.marker_text().contains("call_started:hang") {
        assert!(Instant::now() < deadline, "fixture call did not start");
        thread::sleep(Duration::from_millis(25));
    }

    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("failed to send SIGINT");
    assert!(status.success());

    let output = child.wait_with_output().expect("failed to wait for call");
    assert_eq!(output.status.code(), Some(30));
    let events = json_lines(&output.stdout);
    assert_eq!(events[0]["event"], "execution.started");
    assert!(events
        .iter()
        .any(|event| event["event"] == "execution.cancellation_requested"));
    let error: Value = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(error["event"], "execution.cancelled");
    assert_eq!(error["error"]["code"], "execution_cancelled");
    assert!(
        fixture
            .marker_text()
            .contains("cancelled:cancelled by user (Ctrl+C)"),
        "marker={}",
        fixture.marker_text(),
    );
}

#[test]
fn call_form_accepts_input_file_without_leaking_the_answer() {
    let fixture = Fixture::new("elicitation-form-accept");
    let secret = "form-secret-value";
    let input = fixture.input_file("answer", &json!({"secret": secret, "count": 2}));
    let output = fixture
        .command("elicit_form")
        .args(["--elicitation-input-file"])
        .arg(input)
        .output()
        .expect("failed to run form elicitation call");

    assert_success(&output);
    let events = json_lines(&output.stdout);
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.requested"));
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.accepted"));
    assert!(events
        .iter()
        .any(|event| event["event"] == "execution.completed"));
    let all_output = format!(
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        fixture.marker_text(),
    );
    assert!(
        !all_output.contains(secret),
        "sensitive answer leaked: {all_output}"
    );
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_form:accept"));
}

#[test]
fn call_form_without_input_is_non_interactive_and_requires_input() {
    let fixture = Fixture::new("elicitation-input-required");
    let output = fixture
        .command("elicit_form")
        .output()
        .expect("failed to run input-required elicitation call");

    assert_eq!(output.status.code(), Some(35));
    let events = json_lines(&output.stdout);
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.requested"));
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.input_required"));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "input_required");
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_form:cancel"));
    assert!(fixture
        .marker_text()
        .contains("cancelled:elicitation handling failed"));
}

#[test]
fn call_form_decline_continues_outer_execution() {
    let fixture = Fixture::new("elicitation-decline");
    let output = fixture
        .command("elicit_form")
        .args(["--elicitation-action", "decline"])
        .output()
        .expect("failed to run declined elicitation call");

    assert_success(&output);
    let events = json_lines(&output.stdout);
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.declined"));
    assert!(events
        .iter()
        .any(|event| event["event"] == "execution.completed"));
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_form:decline"));
}

#[test]
fn call_form_cancel_has_stable_exit_and_typed_response() {
    let fixture = Fixture::new("elicitation-cancel");
    let output = fixture
        .command("elicit_form")
        .args(["--elicitation-action", "cancel"])
        .output()
        .expect("failed to run cancelled elicitation call");

    assert_eq!(output.status.code(), Some(36));
    let events = json_lines(&output.stdout);
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.cancelled"));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "elicitation_cancelled");
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_form:cancel"));
}

#[test]
fn call_form_invalid_answer_has_stable_error_without_leaking_the_answer() {
    let fixture = Fixture::new("elicitation-invalid");
    let secret = "invalid-answer-secret";
    let input = fixture.input_file(
        "invalid-answer",
        &json!({"secret": secret, "count": "not-an-integer"}),
    );
    let output = fixture
        .command("elicit_form")
        .args(["--elicitation-input-file"])
        .arg(input)
        .output()
        .expect("failed to run invalid elicitation call");

    assert_eq!(output.status.code(), Some(38));
    let error_text = String::from_utf8_lossy(&output.stderr);
    let error: Value = serde_json::from_slice(output.stderr.as_slice()).unwrap();
    assert_eq!(error["error"]["code"], "elicitation_invalid_response");
    assert!(!error_text.contains(secret));
    assert!(!fixture.marker_text().contains(secret));
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_form:cancel"));
}

#[test]
fn call_url_handoff_is_explicit_and_not_in_generic_events() {
    let fixture = Fixture::new("elicitation-url");
    let output = fixture
        .command("elicit_url")
        .output()
        .expect("failed to run URL elicitation call");

    assert_success(&output);
    let events = json_lines(&output.stdout);
    let requested = events
        .iter()
        .find(|event| event["event"] == "elicitation.requested")
        .unwrap();
    let handoff = events
        .iter()
        .find(|event| event["event"] == "elicitation.url_handoff")
        .unwrap();
    let accepted = events
        .iter()
        .find(|event| event["event"] == "elicitation.accepted")
        .unwrap();
    assert!(requested.get("url").is_none());
    assert!(accepted.get("url").is_none());
    assert_eq!(handoff["url"], "https://example.com/continue");
    assert!(fixture
        .marker_text()
        .contains("elicitation_response:elicit_url:accept"));
}

#[test]
fn call_url_rejects_unsafe_handoff_without_echoing_it() {
    for (tool, rejected) in [
        ("elicit_url_unsafe_scheme", "file:///tmp/secret"),
        (
            "elicit_url_credentials",
            "https://user:secret@example.com/continue",
        ),
    ] {
        let fixture = Fixture::new(tool);
        let output = fixture
            .command(tool)
            .output()
            .unwrap_or_else(|error| panic!("failed to run {tool}: {error}"));
        assert_eq!(output.status.code(), Some(38));
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "elicitation_invalid_response");
        assert!(!text.contains(rejected), "rejected URL leaked: {text}");
        assert!(!fixture.marker_text().contains(rejected));
    }
}

#[test]
fn call_without_non_interactive_flag_does_not_block_on_non_tty() {
    let fixture = Fixture::new("elicitation-no-tty");
    let output = fixture
        .command_with_output_and_interactive("elicit_form", "jsonl", true)
        .output()
        .expect("failed to run non-TTY elicitation call");

    assert_eq!(output.status.code(), Some(35));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "input_required");
}

#[test]
fn task_run_handles_form_elicitation_with_the_same_contract() {
    let fixture = Fixture::new("elicitation-task");
    let input = fixture.input_file("task-answer", &json!({"secret": "task-secret", "count": 3}));
    let output = fixture
        .task_command("elicit_task_form", "jsonl")
        .args(["--elicitation-input-file"])
        .arg(input)
        .output()
        .expect("failed to run task elicitation");

    assert_success(&output);
    let events = json_lines(&output.stdout);
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.requested"));
    assert!(events
        .iter()
        .any(|event| event["event"] == "elicitation.accepted"));
    assert!(events.iter().any(|event| event["event"] == "task.created"));
    let all_output = format!(
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        fixture.marker_text(),
    );
    assert!(!all_output.contains("task-secret"));
}
