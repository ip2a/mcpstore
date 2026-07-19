use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rmcp::{
    model::{
        CallToolRequestParams, ContentBlock, GetPromptRequestParams, ReadResourceRequestParams,
        ResourceContents,
    },
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, ConfigureCommandExt,
        StreamableHttpClientTransport, TokioChildProcess,
    },
    ServiceExt,
};
use tokio::io::AsyncReadExt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
    let dir = std::env::temp_dir().join(format!("mcpstore-mcp-server-flow-{nanos}"));
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

fn reserve_local_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind temp port");
    listener
        .local_addr()
        .expect("failed to read temp port")
        .port()
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

async fn run_test_with_timeout<F>(name: &str, future: F) -> TestResult
where
    F: Future<Output = TestResult>,
{
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .map_err(|_| format!("{name} timed out after 20s"))?
}

#[tokio::test]
async fn mcp_server_command_exposes_store_tools_over_stdio() -> TestResult {
    run_test_with_timeout(
        "mcp_server_command_exposes_store_tools_over_stdio",
        mcp_server_command_exposes_store_tools_over_stdio_inner(),
    )
    .await
}

async fn mcp_server_command_exposes_store_tools_over_stdio_inner() -> TestResult {
    let repo_root = repo_root();
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root.join("python/src").display(),
        repo_root
            .join("rust/apps/mcpstore/tests/fixtures")
            .display()
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

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp-server")
                .arg("--config-path")
                .arg(config_path.display().to_string())
                .current_dir(rust_root());
        }))
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stderr = stderr.expect("stderr must be piped");
    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        stderr.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });

    let client = match ().serve(transport).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!("mcp-server 握手失败: {error}; stderr:\n{stderr_output}").into());
        }
    };
    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), "greet");

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    let resource_uri = resources[0].uri.clone();
    assert!(resource_uri.starts_with("mcpstore://aggregate/"));
    assert!(
        resource_uri.ends_with("fixture:%2F%2Fdocs%2Freadme"),
        "unexpected aggregate URI: {resource_uri}"
    );

    let resource_templates = client.list_all_resource_templates().await?;
    assert!(resource_templates.is_empty());

    let resource = client
        .read_resource(ReadResourceRequestParams::new(resource_uri))
        .await?;
    assert_eq!(resource.contents.len(), 1);
    match &resource.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => {
            assert_eq!(text, "This is the MCPStore fixture resource.");
        }
        other => panic!("unexpected resource content: {other:?}"),
    }

    let prompts = client.list_all_prompts().await?;
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "explain");

    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("explain").with_arguments(
                serde_json::json!({"topic": "stdio"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    assert_eq!(prompt.messages.len(), 1);
    match &prompt.messages[0].content {
        ContentBlock::Text(text) => {
            assert_eq!(text.text, "Explain stdio via fixture prompt.");
        }
        other => panic!("unexpected prompt content: {other:?}"),
    }

    let args: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(serde_json::json!({"name": "World"}))?;
    let result = client
        .call_tool(CallToolRequestParams::new("greet").with_arguments(args))
        .await?;

    let text = result
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|text| text.text.as_str())
        .expect("expected text result");
    assert_eq!(text, "Hello, World!");

    client.cancel().await?;
    let _ = stderr_task.await?;
    Ok(())
}

#[tokio::test]
async fn mcp_server_command_exposes_agent_scope_over_stdio() -> TestResult {
    run_test_with_timeout(
        "mcp_server_command_exposes_agent_scope_over_stdio",
        mcp_server_command_exposes_agent_scope_over_stdio_inner(),
    )
    .await
}

async fn mcp_server_command_exposes_agent_scope_over_stdio_inner() -> TestResult {
    let repo_root = repo_root();
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root.join("python/src").display(),
        repo_root
            .join("rust/apps/mcpstore/tests/fixtures")
            .display()
    );
    let fixture = fixture_script();

    let add_args = vec![
        "add".to_string(),
        "demo".to_string(),
        "--scope".to_string(),
        "agent".to_string(),
        "--agent".to_string(),
        "agent-a".to_string(),
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

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp-server")
                .arg("--scope")
                .arg("agent")
                .arg("--agent")
                .arg("agent-a")
                .arg("--config-path")
                .arg(config_path.display().to_string())
                .current_dir(rust_root());
        }))
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stderr = stderr.expect("stderr must be piped");
    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        stderr.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });

    let client = match ().serve(transport).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!(
                "agent scope mcp-server 握手失败: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), "greet");

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    assert!(resources[0].uri.starts_with("mcpstore://aggregate/"));
    assert!(resources[0].uri.ends_with("fixture:%2F%2Fdocs%2Freadme"));

    let prompts = client.list_all_prompts().await?;
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "explain");

    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("explain").with_arguments(
                serde_json::json!({"topic": "agent"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    assert_eq!(prompt.messages.len(), 1);
    match &prompt.messages[0].content {
        ContentBlock::Text(text) => {
            assert_eq!(text.text, "Explain agent via fixture prompt.");
        }
        other => panic!("unexpected prompt content: {other:?}"),
    }

    client.cancel().await?;
    let _ = stderr_task.await?;
    Ok(())
}

#[tokio::test]
async fn mcp_server_command_exposes_store_tools_over_streamable_http() -> TestResult {
    run_test_with_timeout(
        "mcp_server_command_exposes_store_tools_over_streamable_http",
        mcp_server_command_exposes_store_tools_over_streamable_http_inner(),
    )
    .await
}

async fn mcp_server_command_exposes_store_tools_over_streamable_http_inner() -> TestResult {
    let repo_root = repo_root();
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root.join("python/src").display(),
        repo_root
            .join("rust/apps/mcpstore/tests/fixtures")
            .display()
    );
    let fixture = fixture_script();
    let port = reserve_local_port();

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

    let mut child = tokio::process::Command::new(cli_bin())
        .arg("mcp-server")
        .arg("--transport")
        .arg("streamable-http")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--config-path")
        .arg(config_path.display().to_string())
        .current_dir(rust_root())
        .kill_on_drop(true)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()?;

    let mut stderr = child.stderr.take().expect("stderr must be piped");
    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        stderr.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });

    let base_url = format!("http://127.0.0.1:{port}/mcp");
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill().await;
            let stderr_output = stderr_task.await??;
            return Err(format!(
                "streamable-http MCP server 连接失败: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), "greet");

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    let resource_uri = resources[0].uri.clone();
    assert!(resource_uri.starts_with("mcpstore://aggregate/"));
    assert!(
        resource_uri.ends_with("fixture:%2F%2Fdocs%2Freadme"),
        "unexpected aggregate URI: {resource_uri}"
    );

    let resource_templates = client.list_all_resource_templates().await?;
    assert!(resource_templates.is_empty());

    let resource = client
        .read_resource(ReadResourceRequestParams::new(resource_uri))
        .await?;
    assert_eq!(resource.contents.len(), 1);
    match &resource.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => {
            assert_eq!(text, "This is the MCPStore fixture resource.");
        }
        other => panic!("unexpected resource content: {other:?}"),
    }

    let prompts = client.list_all_prompts().await?;
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "explain");

    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("explain").with_arguments(
                serde_json::json!({"topic": "http"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    assert_eq!(prompt.messages.len(), 1);
    match &prompt.messages[0].content {
        ContentBlock::Text(text) => {
            assert_eq!(text.text, "Explain http via fixture prompt.");
        }
        other => panic!("unexpected prompt content: {other:?}"),
    }

    let args: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(serde_json::json!({"name": "Rust"}))?;
    let result = client
        .call_tool(CallToolRequestParams::new("greet").with_arguments(args))
        .await?;
    let text = result
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|text| text.text.as_str())
        .expect("expected text result");
    assert_eq!(text, "Hello, Rust!");

    client.cancel().await?;
    let _ = child.kill().await;
    let _ = child.wait().await;
    let _ = stderr_task.await?;
    Ok(())
}

async fn connect_http_client(
    base_url: &str,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, Box<dyn std::error::Error>> {
    let mut last_error = String::new();

    for _ in 0..40 {
        let transport = StreamableHttpClientTransport::from_config(
            StreamableHttpClientTransportConfig::with_uri(base_url.to_string()),
        );
        match ().serve(transport).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                last_error = error.to_string();
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    Err(format!("连接 Rust streamable-http MCP server 超时: {last_error}").into())
}
