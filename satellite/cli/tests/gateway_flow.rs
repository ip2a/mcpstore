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
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
};

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
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
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
        .current_dir(repo_root())
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
async fn gateway_projects_and_routes_conflicting_capabilities() -> TestResult {
    run_test_with_timeout(
        "gateway_projects_and_routes_conflicting_capabilities",
        gateway_projects_and_routes_conflicting_capabilities_inner(),
    )
    .await
}

async fn gateway_projects_and_routes_conflicting_capabilities_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
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
            format!(
                "MCPSTORE_FIXTURE_PID_FILE={}",
                temp_dir.join(format!("{service_name}.pid")).display()
            ),
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

    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    let daemon_pid = daemon.pid();
    daemon.connect_service("first", "scope=store").await?;
    daemon.connect_service("second", "scope=store").await?;
    let gateway_child = spawn_gateway(&daemon, &[]).await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "conflict gateway handshake failed: {error}; stderr:\n{stderr_output}"
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
    let upstream_pids = ["first", "second"].map(|name| {
        std::fs::read_to_string(temp_dir.join(format!("{name}.pid"))).expect("upstream fixture pid")
    });
    let _ = gateway_child.shutdown().await;

    let gateway_child = spawn_gateway(&daemon, &[]).await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = connect_http_client(&base_url).await?;
    assert_eq!(daemon.pid(), daemon_pid, "proxy restart killed the daemon");
    for (name, previous) in ["first", "second"].into_iter().zip(upstream_pids) {
        assert_eq!(
            std::fs::read_to_string(temp_dir.join(format!("{name}.pid")))?,
            previous,
            "proxy restart reconnected upstream {name}"
        );
    }

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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_command_exposes_only_selected_instance_over_stdio() -> TestResult {
    run_test_with_timeout(
        "gateway_command_exposes_only_selected_instance_over_stdio",
        gateway_command_exposes_only_selected_instance_over_stdio_inner(),
    )
    .await
}

async fn gateway_command_exposes_only_selected_instance_over_stdio_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
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
    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    daemon.connect_service("selected", "scope=store").await?;
    let gateway_child = spawn_gateway(
        &daemon,
        &["--instance-id".to_string(), instance_id.to_string()],
    )
    .await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "instance gateway handshake failed: {error}; stderr:\n{stderr_output}"
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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_command_exposes_session_scope_over_stdio() -> TestResult {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping session aggregate test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };

    run_test_with_timeout(
        "gateway_command_exposes_session_scope_over_stdio",
        gateway_command_exposes_session_scope_over_stdio_inner(redis_url),
    )
    .await
}

async fn gateway_command_exposes_session_scope_over_stdio_inner(redis_url: String) -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let namespace = format!(
        "mcpstore-mcp-session-flow-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
    );
    let fixture = fixture_script();
    let config_path_arg = config_path.display().to_string();
    let store_args = [
        "--config-path",
        config_path_arg.as_str(),
        "--store",
        "redis",
        "--store-config",
        &format!(r#"{{"url":"{}"}}"#, redis_url),
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

    let daemon_args: Vec<String> = store_args.iter().map(|arg| (*arg).to_string()).collect();
    let daemon = TestDaemon::start(&daemon_args).await?;
    daemon.connect_service("demo", "scope=store").await?;
    let gateway_child = spawn_gateway(
        &daemon,
        &[
            "--session-key".to_string(),
            "store:aggregate-e2e".to_string(),
        ],
    )
    .await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "session gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_exposes_search_meta_tool_when_flag_is_set() -> TestResult {
    run_test_with_timeout(
        "gateway_exposes_search_meta_tool_when_flag_is_set",
        gateway_exposes_search_meta_tool_when_flag_is_set_inner(),
    )
    .await
}

async fn gateway_exposes_search_meta_tool_when_flag_is_set_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
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

    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    daemon.connect_service("demo", "scope=store").await?;
    let gateway_child = spawn_gateway(&daemon, &["--expose-search-tools".to_string()]).await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "search gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_command_exposes_agent_scope_over_stdio() -> TestResult {
    run_test_with_timeout(
        "gateway_command_exposes_agent_scope_over_stdio",
        gateway_command_exposes_agent_scope_over_stdio_inner(),
    )
    .await
}

async fn gateway_command_exposes_agent_scope_over_stdio_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
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

    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    daemon
        .connect_service("demo", "scope=agent&agent_id=agent-a")
        .await?;
    let gateway_child = spawn_gateway(
        &daemon,
        &[
            "--scope".to_string(),
            "agent".to_string(),
            "--agent".to_string(),
            "agent-a".to_string(),
        ],
    )
    .await?;
    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "agent scope gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), tool_name);

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 1);
    assert!(resources[0].uri.starts_with("mcpstore://gateway/"));
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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_command_exposes_store_tools_over_streamable_http() -> TestResult {
    run_test_with_timeout(
        "gateway_command_exposes_store_tools_over_streamable_http",
        gateway_command_exposes_store_tools_over_streamable_http_inner(),
    )
    .await
}

async fn gateway_command_exposes_store_tools_over_streamable_http_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
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

    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    daemon.connect_service("demo", "scope=store").await?;
    let gateway_child = spawn_gateway(&daemon, &[]).await?;

    let base_url = format!("http://127.0.0.1:{}/mcp", gateway_child.port);
    let client = match connect_http_client(&base_url).await {
        Ok(client) => client,
        Err(error) => {
            let details = gateway_child.shutdown().await;
            return Err(format!(
                "streamable-http gateway handshake failed: {error}; stop response:\n{details}"
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
    assert!(resource_uri.starts_with("mcpstore://gateway/"));
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
    let _ = gateway_child.shutdown().await;
    Ok(())
}

/// One isolated daemon per test. Gateway children are owned by this daemon;
/// dropping the fixture kills only that daemon, never the user's real one.
struct TestDaemon {
    child: tokio::process::Child,
    core_url: String,
}

impl TestDaemon {
    async fn start(
        store_args: &[String],
    ) -> Result<std::sync::Arc<Self>, Box<dyn std::error::Error>> {
        let config_path = store_args
            .windows(2)
            .find(|pair| pair[0] == "--config-path")
            .map(|pair| PathBuf::from(&pair[1]))
            .ok_or("test daemon requires --config-path")?;
        let dir = config_path.parent().ok_or("config path has no parent")?;
        let core_port = reserve_local_port();
        std::fs::write(
            dir.join("config.toml"),
            format!("[server]\nhost = \"127.0.0.1\"\nport = {core_port}\ncore_enabled = true\nweb_enabled = false\nrpc_port = 0\n"),
        )?;
        // Keep the Unix socket below SUN_LEN even when macOS gives the test a
        // long per-user temp directory.
        let socket = PathBuf::from(format!(
            "/tmp/mcpstore-gw-{}-{core_port}.sock",
            std::process::id()
        ));
        let pid = dir.join("kernel.pid");
        let home = dir.join("home");
        std::fs::create_dir_all(&home)?;
        let mut child = tokio::process::Command::new(cli_bin())
            .arg("start")
            .args(store_args)
            .current_dir(repo_root())
            .env("MCPSTORE_SOCKET", &socket)
            .env("MCPSTORE_PID", &pid)
            .env("HOME", &home)
            .env_remove("DYLD_FALLBACK_LIBRARY_PATH")
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        let core_url = format!("http://127.0.0.1:{core_port}");
        let client = reqwest::Client::new();
        for _ in 0..100 {
            if let Some(status) = child.try_wait()? {
                return Err(format!("test daemon exited with {status}").into());
            }
            if client
                .get(format!("{core_url}/health"))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                return Ok(std::sync::Arc::new(Self { child, core_url }));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err("test daemon did not become ready".into())
    }

    fn pid(&self) -> u32 {
        self.child.id().expect("daemon pid")
    }

    async fn connect_service(&self, service_name: &str, scope_query: &str) -> TestResult {
        let client = reqwest::Client::new();
        let base = format!("{}/services/{service_name}", self.core_url);
        let response = client
            .post(format!("{base}/connect?{scope_query}"))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(format!("service connect failed: {}", response.text().await?).into());
        }
        for _ in 0..100 {
            let state: serde_json::Value = client
                .get(format!("{base}/state?{scope_query}"))
                .send()
                .await?
                .json()
                .await?;
            match state["data"]["phase"].as_str() {
                Some("running") => return Ok(()),
                Some("failed") => return Err(format!("service failed to connect: {state}").into()),
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        Err(format!("service {service_name} did not become ready").into())
    }
}

struct GatewayChild {
    daemon: std::sync::Arc<TestDaemon>,
    port: u16,
}

async fn spawn_gateway(
    daemon: &std::sync::Arc<TestDaemon>,
    args: &[String],
) -> Result<GatewayChild, Box<dyn std::error::Error>> {
    let port = reserve_local_port();
    let mut url = reqwest::Url::parse(&format!("{}/gateway/start", daemon.core_url))?;
    let mut query = vec![("port", port.to_string())];
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = if flag.starts_with("--expose-") {
            index += 1;
            None
        } else {
            index += 1;
            let value = args.get(index).ok_or("missing gateway test argument")?;
            index += 1;
            Some(value.as_str())
        };
        let key = match flag {
            "--scope" => "scope",
            "--agent" => "agent_id",
            "--instance-id" => "instance_id",
            "--session-key" => "session_key",
            "--expose-session-state-tools" => "expose_session_state",
            "--expose-tool-override-tools" => "expose_tool",
            "--expose-prompt-override-tools" => "expose_prompt",
            "--expose-resource-override-tools" => "expose_resource",
            "--expose-openapi-tools" => "expose_openapi",
            "--expose-service-tools" => "expose_service",
            "--expose-kv-tools" => "expose_kv",
            "--expose-search-tools" => "expose_search",
            other => return Err(format!("unsupported gateway test argument: {other}").into()),
        };
        query.push((key, value.unwrap_or("true").to_string()));
    }
    url.query_pairs_mut().extend_pairs(query);
    let client = reqwest::Client::new();
    let response = client.post(url).send().await?;
    if !response.status().is_success() {
        return Err(format!("gateway start failed: {}", response.text().await?).into());
    }
    for _ in 0..200 {
        let status: serde_json::Value = client
            .get(format!("{}/gateway/status?port={port}", daemon.core_url))
            .send()
            .await?
            .json()
            .await?;
        if status["data"]["running"] == true {
            return Ok(GatewayChild {
                daemon: std::sync::Arc::clone(daemon),
                port,
            });
        }
        if status["data"]["boot"]["phase"] == "failed" {
            return Err(format!("gateway boot failed: {status}").into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err("gateway proxy did not become ready".into())
}

impl GatewayChild {
    async fn shutdown(self) -> String {
        match reqwest::Client::new()
            .post(format!("{}/gateway/stop", self.daemon.core_url))
            .send()
            .await
        {
            Ok(response) => response.text().await.unwrap_or_default(),
            Err(error) => error.to_string(),
        }
    }
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
