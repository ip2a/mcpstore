use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcpstore"))
}

fn run_invalid_input(output: &str) -> Output {
    Command::new(cli_bin())
        .args([
            "task",
            "run",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "long_tool",
            "--input",
            "[]",
            "--output",
            output,
            "--non-interactive",
        ])
        .output()
        .expect("failed to run mcpstore task command")
}

#[test]
fn task_invalid_input_has_stable_machine_error_contract() {
    for output_format in ["json", "jsonl"] {
        let output = run_invalid_input(output_format);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["event"], "task.error");
        assert_eq!(error["error"]["code"], "invalid_input");
        assert_eq!(error["instance_id"], Value::Null);
        assert_eq!(error["task_id"], Value::Null);
    }
}

fn run_invalid_call(output: &str) -> Output {
    Command::new(cli_bin())
        .args([
            "call",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "long_tool",
            "--arguments",
            "[]",
            "--output",
            output,
            "--non-interactive",
        ])
        .output()
        .expect("failed to run mcpstore call command")
}

#[test]
fn call_invalid_input_has_stable_machine_error_contract() {
    for output_format in ["json", "jsonl"] {
        let output = run_invalid_call(output_format);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["event"], "execution.failed");
        assert_eq!(error["error"]["code"], "invalid_input");
        assert_eq!(error["instance_id"], Value::Null);
        assert_eq!(error["tool_name"], Value::Null);
    }
}
