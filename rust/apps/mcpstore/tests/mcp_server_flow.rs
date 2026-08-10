use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rmcp::{
    model::{
        CallToolRequestParams, ContentBlock, GetPromptRequestParams, ProtocolVersion,
        ReadResourceRequestParams, ResourceContents,
    },
    service::{ClientLifecycleMode, ClientServiceExt},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, ConfigureCommandExt,
        StreamableHttpClientTransport, TokioChildProcess,
    },
};
use tokio::io::AsyncReadExt;

use mcpstore::{
    CreateSessionRequest, JsonStoreConfig, MCPStore, ScopeRef, ServiceInstanceKey,
    SessionToolSelection, StoreOptions,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn discover_lifecycle() -> ClientLifecycleMode {
    ClientLifecycleMode::Discover {
        preferred_versions: vec![ProtocolVersion::V_2026_07_28],
    }
}

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
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "mcpstore-mcp-flow-{}-{nanos}-{id}",
        std::process::id()
    ));
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
async fn mcp_server_projects_and_routes_conflicting_capabilities() -> TestResult {
    run_test_with_timeout(
        "mcp_server_projects_and_routes_conflicting_capabilities",
        mcp_server_projects_and_routes_conflicting_capabilities_inner(),
    )
    .await
}

async fn mcp_server_projects_and_routes_conflicting_capabilities_inner() -> TestResult {
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

    for (service_name, label) in [("first", "FIRST"), ("second", "SECOND")] {
        let add_args = vec![
            "add".to_string(),
            service_name.to_string(),
            "--config-path".to_string(),
            config_path.display().to_string(),
            "--transport".to_string(),
            "stdio".to_string(),
            "--env".to_string(),
            format!("PYTHONPATH={pythonpath}"),
            "--env".to_string(),
            format!("MCPSTORE_FIXTURE_LABEL={label}"),
            "--env".to_string(),
            "MCPSTORE_FIXTURE_TEMPLATE=1".to_string(),
            "--env".to_string(),
            format!("MCPSTORE_PRIVATE_TOKEN={label}_ENV_SECRET"),
            "--header".to_string(),
            format!("Authorization=Bearer-{label}_HEADER_SECRET"),
            "--".to_string(),
            "python3".to_string(),
            fixture.display().to_string(),
        ];
        assert_success(&run_cli(&add_args), &format!("add {service_name}"));
    }

    let first_id = ServiceInstanceKey::new("first", ScopeRef::Store).instance_id();
    let second_id = ServiceInstanceKey::new("second", ScopeRef::Store).instance_id();
    let first_namespace = "first";
    let second_namespace = "second";
    let first_prompt_namespace = format!("first__{first_id}");
    let second_prompt_namespace = format!("second__{second_id}");

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
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
    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!(
                "conflict mcp handshake failed: {error}; stderr:\
{stderr_output}"
            )
            .into());
        }
    };

    let tools = client.list_all_tools().await?;
    let tool_names = tools
        .iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    let first_tool = format!("{first_namespace}__greet");
    let second_tool = format!("{second_namespace}__greet");
    assert_eq!(tool_names, vec![first_tool.clone(), second_tool.clone()]);
    for (tool_name, expected) in [
        (first_tool, "FIRST: Hello, Conflict!"),
        (second_tool, "SECOND: Hello, Conflict!"),
    ] {
        let result = client
            .call_tool(
                CallToolRequestParams::new(tool_name).with_arguments(
                    serde_json::json!({"name": "Conflict"})
                        .as_object()
                        .cloned()
                        .unwrap(),
                ),
            )
            .await?;
        assert_eq!(
            result
                .content
                .first()
                .and_then(ContentBlock::as_text)
                .map(|text| text.text.as_str()),
            Some(expected)
        );
    }

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 4);
    for (namespace, expected) in [
        (
            &first_namespace,
            "FIRST: This is the MCPStore fixture resource.",
        ),
        (
            &second_namespace,
            "SECOND: This is the MCPStore fixture resource.",
        ),
    ] {
        let resource = resources
            .iter()
            .find(|resource| resource.uri.contains(namespace) && resource.uri.ends_with("readme"))
            .expect("projected resource must include its stable namespace");
        let result = client
            .read_resource(ReadResourceRequestParams::new(resource.uri.clone()))
            .await?;
        match &result.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => assert_eq!(text, expected),
            other => panic!("unexpected resource content: {other:?}"),
        }
    }

    let projected_templates = resources
        .iter()
        .filter(|resource| resource.uri.contains("%7Bname%7D"))
        .collect::<Vec<_>>();
    assert_eq!(projected_templates.len(), 2);
    assert!(projected_templates
        .iter()
        .any(|resource| resource.uri.contains(&first_namespace)));
    assert!(projected_templates
        .iter()
        .any(|resource| resource.uri.contains(&second_namespace)));

    let prompts = client.list_all_prompts().await?;
    let public_catalog = serde_json::to_string(&(&tools, &resources, &prompts))?;
    for secret in [
        "FIRST_ENV_SECRET",
        "SECOND_ENV_SECRET",
        "FIRST_HEADER_SECRET",
        "SECOND_HEADER_SECRET",
        "MCPSTORE_PRIVATE_TOKEN",
        "Authorization",
        "PYTHONPATH",
        fixture.to_string_lossy().as_ref(),
    ] {
        assert!(
            !public_catalog.contains(secret),
            "aggregate catalog leaked sensitive configuration marker: {secret}"
        );
    }
    let prompt_names = prompts
        .iter()
        .map(|prompt| prompt.name.clone())
        .collect::<Vec<_>>();
    let first_prompt = format!("{first_prompt_namespace}__explain");
    let second_prompt = format!("{second_prompt_namespace}__explain");
    assert_eq!(
        prompt_names,
        vec![first_prompt.clone(), second_prompt.clone()]
    );
    for (prompt_name, expected) in [
        (first_prompt, "FIRST: Explain conflict via fixture prompt."),
        (
            second_prompt,
            "SECOND: Explain conflict via fixture prompt.",
        ),
    ] {
        let prompt = client
            .get_prompt(
                GetPromptRequestParams::new(prompt_name).with_arguments(
                    serde_json::json!({"topic": "conflict"})
                        .as_object()
                        .cloned()
                        .unwrap(),
                ),
            )
            .await?;
        match &prompt.messages[0].content {
            ContentBlock::Text(text) => assert_eq!(text.text, expected),
            other => panic!("unexpected prompt content: {other:?}"),
        }
    }

    let resource_uris = resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect::<Vec<_>>();
    client.cancel().await?;
    let _ = stderr_task.await?;

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
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
    let client = ().serve_with_lifecycle(transport, discover_lifecycle()).await?;

    assert_eq!(
        client
            .list_all_tools()
            .await?
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>(),
        tool_names
    );
    assert_eq!(
        client
            .list_all_resources()
            .await?
            .into_iter()
            .map(|resource| resource.uri)
            .collect::<Vec<_>>(),
        resource_uris
    );
    assert_eq!(
        client
            .list_all_prompts()
            .await?
            .into_iter()
            .map(|prompt| prompt.name)
            .collect::<Vec<_>>(),
        prompt_names
    );

    client.cancel().await?;
    let _ = stderr_task.await?;
    Ok(())
}

#[tokio::test]
async fn mcp_server_command_exposes_only_selected_instance_over_stdio() -> TestResult {
    run_test_with_timeout(
        "mcp_server_command_exposes_only_selected_instance_over_stdio",
        mcp_server_command_exposes_only_selected_instance_over_stdio_inner(),
    )
    .await
}

async fn mcp_server_command_exposes_only_selected_instance_over_stdio_inner() -> TestResult {
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

    for service_name in ["selected", "hidden"] {
        let add_args = vec![
            "add".to_string(),
            service_name.to_string(),
            "--config-path".to_string(),
            config_path.display().to_string(),
            "--transport".to_string(),
            "stdio".to_string(),
            "--env".to_string(),
            format!("PYTHONPATH={pythonpath}"),
            "--".to_string(),
            "python3".to_string(),
            fixture.display().to_string(),
        ];
        assert_success(&run_cli(&add_args), &format!("add {service_name}"));
    }

    let instance_id = ServiceInstanceKey::new("selected", ScopeRef::Store).instance_id();
    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
                .arg("--config-path")
                .arg(config_path.display().to_string())
                .arg("--instance-id")
                .arg(instance_id.to_string())
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
    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!(
                "instance mcp handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), "greet");
    let result = client
        .call_tool(
            CallToolRequestParams::new("greet").with_arguments(
                serde_json::json!({"name": "Instance"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    assert_eq!(
        result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .map(|text| text.text.as_str()),
        Some("Hello, Instance!")
    );

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    assert!(resources[0].uri.contains(&instance_id.to_string()));
    let resource = client
        .read_resource(ReadResourceRequestParams::new(resources[0].uri.clone()))
        .await?;
    match &resource.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => {
            assert_eq!(text, "This is the MCPStore fixture resource.");
        }
        other => panic!("unexpected resource content: {other:?}"),
    }

    assert!(client.list_all_resource_templates().await?.is_empty());
    let prompts = client.list_all_prompts().await?;
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "explain");
    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("explain").with_arguments(
                serde_json::json!({"topic": "instance"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    match &prompt.messages[0].content {
        ContentBlock::Text(text) => {
            assert_eq!(text.text, "Explain instance via fixture prompt.");
        }
        other => panic!("unexpected prompt content: {other:?}"),
    }

    client.cancel().await?;
    let _ = stderr_task.await?;
    Ok(())
}

#[tokio::test]
async fn mcp_server_command_exposes_session_scope_over_stdio() -> TestResult {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping session aggregate test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };

    run_test_with_timeout(
        "mcp_server_command_exposes_session_scope_over_stdio",
        mcp_server_command_exposes_session_scope_over_stdio_inner(redis_url),
    )
    .await
}

async fn mcp_server_command_exposes_session_scope_over_stdio_inner(
    redis_url: String,
) -> TestResult {
    let repo_root = repo_root();
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let namespace = format!(
        "mcpstore-mcp-session-flow-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let pythonpath = format!(
        "{}:{}",
        repo_root.join("python/src").display(),
        repo_root
            .join("rust/apps/mcpstore/tests/fixtures")
            .display()
    );
    let fixture = fixture_script();
    let config_path_arg = config_path.display().to_string();
    let store_args = [
        "--config-path",
        config_path_arg.as_str(),
        "--redis-url",
        redis_url.as_str(),
        "--namespace",
        namespace.as_str(),
    ];

    let mut add_args = vec!["add".to_string(), "demo".to_string()];
    add_args.extend(store_args.iter().map(|arg| (*arg).to_string()));
    add_args.extend([
        "--transport".to_string(),
        "stdio".to_string(),
        "--env".to_string(),
        format!("PYTHONPATH={pythonpath}"),
        "--".to_string(),
        "python3".to_string(),
        fixture.display().to_string(),
    ]);
    let add_stdout = assert_success(&run_cli(&add_args), "add session fixture");
    assert!(add_stdout.contains("[Success] Service added: demo"));

    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(config_path_arg.clone()),
        store: Some(JsonStoreConfig::redis(&redis_url)),
        namespace: Some(namespace.clone()),
        ..StoreOptions::default()
    })?;
    store.load_from_source().await?;
    let instance_id = store
        .instance_id_for_scope("demo", &ScopeRef::Store)
        .await?;
    let session = store
        .create_session(CreateSessionRequest::store("aggregate-e2e"))
        .await?;
    let session = store.session_by_key(session.session_key.clone());
    session.bind_service(instance_id).await?;
    session
        .set_tool_visibility(vec![SessionToolSelection {
            instance_id,
            tool_name: "greet".to_string(),
        }])
        .await?;

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
                .args(store_args)
                .arg("--session-key")
                .arg("store:aggregate-e2e")
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
    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(
                format!("session mcp handshake failed: {error}; stderr:\n{stderr_output}").into(),
            );
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), "greet");

    let args = serde_json::json!({"name": "Session"})
        .as_object()
        .cloned()
        .unwrap();
    let result = client
        .call_tool(CallToolRequestParams::new("greet").with_arguments(args))
        .await?;
    assert_eq!(
        result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .map(|text| text.text.as_str()),
        Some("Hello, Session!")
    );

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    let resource = client
        .read_resource(ReadResourceRequestParams::new(resources[0].uri.clone()))
        .await?;
    match &resource.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => {
            assert_eq!(text, "This is the MCPStore fixture resource.");
        }
        other => panic!("unexpected resource content: {other:?}"),
    }

    let prompts = client.list_all_prompts().await?;
    assert_eq!(prompts.len(), 1);
    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new(prompts[0].name.clone()).with_arguments(
                serde_json::json!({"topic": "session"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    match &prompt.messages[0].content {
        ContentBlock::Text(text) => {
            assert_eq!(text.text, "Explain session via fixture prompt.");
        }
        other => panic!("unexpected prompt content: {other:?}"),
    }

    client.cancel().await?;
    let _ = stderr_task.await?;
    Ok(())
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
        "python3".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));
    let tool_name = "demo__greet".to_string();

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
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

    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!("mcp 握手失败: {error}; stderr:\n{stderr_output}").into());
        }
    };
    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), tool_name);

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
        .call_tool(CallToolRequestParams::new(tool_name).with_arguments(args))
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
async fn mcp_server_exposes_search_meta_tool_when_flag_is_set() -> TestResult {
    run_test_with_timeout(
        "mcp_server_exposes_search_meta_tool_when_flag_is_set",
        mcp_server_exposes_search_meta_tool_when_flag_is_set_inner(),
    )
    .await
}

async fn mcp_server_exposes_search_meta_tool_when_flag_is_set_inner() -> TestResult {
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
        "python3".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
                .arg("--config-path")
                .arg(config_path.display().to_string())
                .arg("--expose-search-tools")
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

    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(format!("mcp 握手失败: {error}; stderr:\n{stderr_output}").into());
        }
    };

    // The search meta-tool surfaces alongside the downstream tool only when the
    // flag is on (other tests assert it is absent by default).
    let tools = client.list_all_tools().await?;
    let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(tool_names.contains(&"demo__greet".to_string()));
    assert!(tool_names.contains(&"mcpstore_search_tools".to_string()));

    // Searching for "greet" returns the downstream tool; searching for nonsense
    // returns nothing.
    let hit = client
        .call_tool(
            CallToolRequestParams::new("mcpstore_search_tools").with_arguments(
                serde_json::json!({"query": "greet", "top_k": 5})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    let hit_text = hit
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|t| t.text.as_str().to_string())
        .expect("search result must be text");
    assert!(
        hit_text.contains("demo__greet"),
        "search for 'greet' must surface demo__greet: {hit_text}"
    );

    let miss = client
        .call_tool(
            CallToolRequestParams::new("mcpstore_search_tools").with_arguments(
                serde_json::json!({"query": "zzzznotoolsmatchthis"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    let miss_text = miss
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|t| t.text.as_str().to_string())
        .expect("search result must be text");
    assert!(
        !miss_text.contains("demo__greet"),
        "search for unrelated term must not surface demo__greet: {miss_text}"
    );

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
        "python3".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));
    let tool_name = "demo__greet".to_string();

    let (transport, stderr) =
        TokioChildProcess::builder(tokio::process::Command::new(cli_bin()).configure(|cmd| {
            cmd.arg("mcp")
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

    let client = match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = stderr_task.await??;
            return Err(
                format!("agent scope mcp 握手失败: {error}; stderr:\n{stderr_output}").into(),
            );
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), tool_name);

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
        "python3".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));
    let tool_name = "demo__greet".to_string();

    let mut child = tokio::process::Command::new(cli_bin())
        .arg("mcp")
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
    assert_eq!(tools[0].name.as_ref(), tool_name);

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
        .call_tool(CallToolRequestParams::new(tool_name).with_arguments(args))
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
        match ().serve_with_lifecycle(transport, discover_lifecycle()).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                last_error = error.to_string();
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    Err(format!("连接 Rust streamable-http MCP server 超时: {last_error}").into())
}
