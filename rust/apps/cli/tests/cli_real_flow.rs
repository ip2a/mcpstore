use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

fn rust_root() -> PathBuf {
    repo_root().join("rust")
}

fn fixture_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mock_mcp_server.py")
        .canonicalize()
        .expect("failed to resolve fixture server path")
}

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcpstore"))
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mcpstore-cli-real-flow-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn run_cli(args: &[String]) -> Output {
    Command::new(cli_bin())
        .args(args)
        .current_dir(rust_root())
        .output()
        .expect("failed to run mcpstore cli")
}

fn assert_success(output: &Output, step: &str) -> String {
    if !output.status.success() {
        panic!(
            "{step} failed\nstatus={}\nstdout=\n{}\nstderr=\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
#[ignore = "integration test: stdio transport environment variance causes hangs in debug builds"]
fn cli_binary_completes_real_stdio_service_flow() {
    let repo_root = repo_root();
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root.join("python/src").display(),
        repo_root.join("rust/apps/cli/tests/fixtures").display()
    );
    let fixture = fixture_script();

    let add_args = vec![
        "add".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
        "--env".to_string(),
        format!("PYTHONPATH={pythonpath}"),
        "--".to_string(),
        "uv".to_string(),
        "run".to_string(),
        "--project".to_string(),
        repo_root.join("python").display().to_string(),
        "python".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));

    let list_args = vec![
        "list".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let list_stdout = assert_success(&run_cli(&list_args), "list");
    assert!(list_stdout.contains("[List] service_count=1"));
    assert!(list_stdout.contains("- demo  transport=stdio"));

    let get_args = vec![
        "get".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let get_stdout = assert_success(&run_cli(&get_args), "get");
    assert!(get_stdout.contains("\"name\": \"demo\""));
    assert!(get_stdout.contains("\"transport\": \"stdio\""));

    let connect_args = vec![
        "connect".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let connect_stdout = assert_success(&run_cli(&connect_args), "connect");
    assert!(connect_stdout.contains("[Success] Connected: demo (tools=1)"));
    assert!(connect_stdout.contains("greet: Greet caller."));

    let tools_args = vec![
        "tools".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let tools_stdout = assert_success(&run_cli(&tools_args), "tools");
    assert!(tools_stdout.contains("[Tools] service=demo count=1"));
    assert!(tools_stdout.contains("greet: Greet caller."));

    let call_args = vec![
        "call".to_string(),
        "demo".to_string(),
        "greet".to_string(),
        "--arguments".to_string(),
        r#"{"name":"World"}"#.to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let call_stdout = assert_success(&run_cli(&call_args), "call");
    assert!(call_stdout.contains("Hello, World!"));

    let disconnect_args = vec![
        "disconnect".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let disconnect_stdout = assert_success(&run_cli(&disconnect_args), "disconnect");
    assert!(disconnect_stdout.contains("[Success] Disconnected: demo"));

    let remove_args = vec![
        "remove".to_string(),
        "demo".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let remove_stdout = assert_success(&run_cli(&remove_args), "remove");
    assert!(remove_stdout.contains("[Success] Service removed: demo"));

    let final_list_stdout = assert_success(&run_cli(&list_args), "final list");
    assert!(final_list_stdout.contains("[List] service_count=0"));
}
