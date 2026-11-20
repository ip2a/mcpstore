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

// Wall-clock cap only guards against hangs (daemon never binds, fixture
// never answers). Full flows spawn daemon + Python fixture + gateway and
// run alongside ~9 sibling tests; under that load 20s tripped legitimate
// runs (~15% flake), so the cap sits well above worst-case healthy time.
const TEST_TIMEOUT_SECS: u64 = 120;

async fn run_test_with_timeout<F>(name: &str, future: F) -> TestResult
where
    F: Future<Output = TestResult>,
{
    tokio::time::timeout(Duration::from_secs(TEST_TIMEOUT_SECS), future)
        .await
        .map_err(|_| format!("{name} timed out after {TEST_TIMEOUT_SECS}s"))?
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
    // Same exposed name from both services merges into one list entry; calls
    // pick a max-weight candidate, and equal weights tie randomly.
    assert_eq!(tool_names, vec!["greet".to_string()]);
    let result = client
        .call_tool(
            CallToolRequestParams::new("greet").with_arguments(
                serde_json::json!({"name": "Conflict"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    let greeting = result
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|text| text.text.to_string());
    assert!(
        greeting.as_deref() == Some("FIRST: Hello, Conflict!")
            || greeting.as_deref() == Some("SECOND: Hello, Conflict!"),
        "merged greet must route to one of the two upstream candidates, got {greeting:?}"
    );

    let resources = client.list_all_resources().await?;
    assert_eq!(resources.len(), 2);
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

    let projected_templates = client.list_all_resource_templates().await?;
    assert_eq!(projected_templates.len(), 2);
    assert!(projected_templates
        .iter()
        .any(|resource| resource.uri_template.contains(&first_namespace)
            && resource.uri_template.contains("%7Bname%7D")));
    assert!(projected_templates
        .iter()
        .any(|resource| resource.uri_template.contains(&second_namespace)
            && resource.uri_template.contains("%7Bname%7D")));

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

/// Shared `mcpstore add` arguments for the fixture server plus the
/// catalog-scale knobs the search tests need.
fn fixture_add_args(
    config_path: &Path,
    service_name: &str,
    extra_env: &[(&str, String)],
) -> Vec<String> {
    let pythonpath = format!(
        "{}:{}",
        repo_root().join("satellite/python").display(),
        repo_root().join("satellite/cli/tests/fixtures").display()
    );
    let mut args = vec![
        "add".to_string(),
        service_name.to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
        "--env".to_string(),
        format!("PYTHONPATH={pythonpath}"),
    ];
    for (key, value) in extra_env {
        args.push("--env".to_string());
        args.push(format!("{key}={value}"));
    }
    args.push("--".to_string());
    args.push("python3".to_string());
    args.push(fixture_script().display().to_string());
    args
}

fn call_args(arguments: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    arguments.as_object().cloned().unwrap()
}

#[tokio::test]
async fn gateway_search_flag_off_exposes_no_helper_names() -> TestResult {
    run_test_with_timeout(
        "gateway_search_flag_off_exposes_no_helper_names",
        gateway_search_flag_off_exposes_no_helper_names_inner(),
    )
    .await
}

async fn gateway_search_flag_off_exposes_no_helper_names_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    assert_success(
        &run_cli(&fixture_add_args(&config_path, "demo", &[])),
        "add demo",
    );

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
            let stderr_output = gateway_child.shutdown().await;
            return Err(format!(
                "search-off gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    let names: Vec<String> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names, vec!["greet".to_string()]);

    // With the flag off the helper names are not declared and not
    // dispatchable — a hand-written call falls through to unknown-tool.
    for helper in ["search_tools", "search_prompt", "search_resource"] {
        let error = client
            .call_tool(
                CallToolRequestParams::new(helper)
                    .with_arguments(call_args(serde_json::json!({"query": "greet"}))),
            )
            .await;
        assert!(error.is_err(), "inactive {helper} must be an unknown tool");
    }

    client.cancel().await?;
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_search_helpers_activate_per_kind() -> TestResult {
    run_test_with_timeout(
        "gateway_search_helpers_activate_per_kind",
        gateway_search_helpers_activate_per_kind_inner(),
    )
    .await
}

/// Acceptance vector (tools, prompts, resources, templates) = (9, 10, 6, 4):
/// only search_prompt and search_resource may appear, and a hot search never
/// touches the upstream.
async fn gateway_search_helpers_activate_per_kind_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    let call_log_path = temp_dir.join("fixture-calls.log");
    assert_success(
        &run_cli(
            &fixture_add_args(
                &config_path,
                "demo",
                &[
                    ("MCPSTORE_FIXTURE_EXTRA_TOOLS", "8".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_PROMPTS", "9".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_RESOURCES", "5".to_string()),
                    ("MCPSTORE_FIXTURE_TEMPLATES", "4".to_string()),
                    (
                        "MCPSTORE_FIXTURE_CALL_LOG",
                        call_log_path.display().to_string(),
                    ),
                ],
            ),
        ),
        "add demo",
    );

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

    // Cold mount: only the tools snapshot is known, no helper may appear.
    let names: Vec<String> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let mut business = names.clone();
    business.sort();
    assert_eq!(
        business,
        (0..8)
            .map(|index| format!("extra_tool_{index}"))
            .chain(["greet".to_string()])
            .collect::<Vec<_>>()
    );

    // Complete native pages publish the discovery rows: 10 prompts activate
    // search_prompt; 6 resources + 4 templates activate search_resource. Nine
    // tools stay below the threshold, so search_tools never appears.
    assert_eq!(client.list_all_prompts().await?.len(), 10);
    assert_eq!(client.list_all_resources().await?.len(), 6);
    assert_eq!(client.list_all_resource_templates().await?.len(), 4);
    let names: Vec<String> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let mut expected = business.clone();
    expected.push("search_prompt".to_string());
    expected.push("search_resource".to_string());
    expected.sort();
    assert_eq!(names, expected);

    // search_prompt: prompt hit carries the public name and no unknownKinds.
    let hit = client
        .call_tool(
            CallToolRequestParams::new("search_prompt").with_arguments(call_args(
                serde_json::json!({"query": "extra prompt three"}),
            )),
        )
        .await?;
    let structured = hit.structured_content.expect("structured payload");
    assert_eq!(structured["items"][0]["kind"], "prompt");
    assert_eq!(structured["items"][0]["name"], "extra_prompt_3");
    assert_eq!(structured["unknownKinds"], serde_json::json!([]));

    // limit is honored, misses stay empty.
    let limited = client
        .call_tool(
            CallToolRequestParams::new("search_prompt").with_arguments(call_args(
                serde_json::json!({"query": "extra", "limit": 2}),
            )),
        )
        .await?;
    assert!(
        limited.structured_content.expect("structured payload")["items"]
            .as_array()
            .expect("items array")
            .len()
            <= 2
    );
    let miss = client
        .call_tool(
            CallToolRequestParams::new("search_prompt").with_arguments(call_args(
                serde_json::json!({"query": "zzzznothingmatchesthis"}),
            )),
        )
        .await?;
    assert!(miss.structured_content.expect("structured payload")["items"]
        .as_array()
        .expect("items array")
        .is_empty());

    // search_resource: a concrete resource hit is readable through its
    // returned public uri; a template hit reports uriTemplate only.
    let resource_hit = client
        .call_tool(
            CallToolRequestParams::new("search_resource").with_arguments(call_args(
                serde_json::json!({"query": "readme"}),
            )),
        )
        .await?;
    let structured = resource_hit.structured_content.expect("structured payload");
    assert_eq!(structured["items"][0]["kind"], "resource");
    let uri = structured["items"][0]["uri"]
        .as_str()
        .expect("resource uri")
        .to_string();
    let read = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await?;
    match &read.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => {
            assert_eq!(text, "This is the MCPStore fixture resource.");
        }
        other => panic!("unexpected resource content: {other:?}"),
    }
    let template_hit = client
        .call_tool(
            CallToolRequestParams::new("search_resource").with_arguments(call_args(
                serde_json::json!({"query": "template 2"}),
            )),
        )
        .await?;
    let structured = template_hit.structured_content.expect("structured payload");
    assert_eq!(structured["items"][0]["kind"], "resource_template");
    assert!(structured["items"][0].get("uriTemplate").is_some());

    // Below-threshold search_tools is not dispatchable.
    let error = client
        .call_tool(
            CallToolRequestParams::new("search_tools").with_arguments(call_args(
                serde_json::json!({"query": "greet"}),
            )),
        )
        .await;
    assert!(error.is_err(), "9 tools must not activate search_tools");

    // Strict input contract: retired top_k and a foreign session key are
    // both invalid_params, never silently tolerated.
    for bad in [
        serde_json::json!({"query": "x", "top_k": 3}),
        serde_json::json!({"query": "", "limit": 0}),
        serde_json::json!({"query": "x", "_mcpstore_session_key": "store:elsewhere"}),
    ] {
        let error = client
            .call_tool(
                CallToolRequestParams::new("search_prompt").with_arguments(call_args(bad)),
            )
            .await;
        assert!(error.is_err(), "invalid arguments must be rejected: {error:?}");
    }

    // Zero extra upstream access from hot searches: the fixture logs every
    // tool/prompt/resource invocation, and none of the searches may add lines.
    // 50 iterations × 2 kinds = the 100 hot searches the acceptance calls for.
    let log_before = std::fs::read_to_string(&call_log_path).unwrap_or_default();
    for _ in 0..50 {
        client
            .call_tool(
                CallToolRequestParams::new("search_prompt").with_arguments(call_args(
                    serde_json::json!({"query": "explain"}),
                )),
            )
            .await?;
        client
            .call_tool(
                CallToolRequestParams::new("search_resource").with_arguments(call_args(
                    serde_json::json!({"query": "extra resource"}),
                )),
            )
            .await?;
    }
    let log_after = std::fs::read_to_string(&call_log_path).unwrap_or_default();
    assert_eq!(
        log_before, log_after,
        "search helpers must not invoke any upstream tool/prompt/resource"
    );

    client.cancel().await?;
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_search_tools_helper_needs_ten_tools() -> TestResult {
    run_test_with_timeout(
        "gateway_search_tools_helper_needs_ten_tools",
        gateway_search_tools_helper_needs_ten_tools_inner(),
    )
    .await
}

/// Same fixture at (10, 10, 6, 4): all three helpers appear, and a hit tool
/// name round-trips through tools/call.
async fn gateway_search_tools_helper_needs_ten_tools_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    assert_success(
        &run_cli(
            &fixture_add_args(
                &config_path,
                "demo",
                &[
                    ("MCPSTORE_FIXTURE_EXTRA_TOOLS", "9".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_PROMPTS", "9".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_RESOURCES", "5".to_string()),
                    ("MCPSTORE_FIXTURE_TEMPLATES", "4".to_string()),
                ],
            ),
        ),
        "add demo",
    );

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

    assert_eq!(client.list_all_prompts().await?.len(), 10);
    assert_eq!(client.list_all_resources().await?.len(), 6);
    assert_eq!(client.list_all_resource_templates().await?.len(), 4);

    let hit = client
        .call_tool(
            CallToolRequestParams::new("search_tools").with_arguments(call_args(
                serde_json::json!({"query": "extra tool four"}),
            )),
        )
        .await?;
    let structured = hit.structured_content.expect("structured payload");
    assert_eq!(structured["items"][0]["kind"], "tool");
    let name = structured["items"][0]["name"]
        .as_str()
        .expect("tool name")
        .to_string();
    assert_eq!(name, "extra_tool_4");

    // The hit name is a normal public tool name.
    let result = client
        .call_tool(
            CallToolRequestParams::new(name.clone())
                .with_arguments(call_args(serde_json::json!({"query": "Search"}))),
        )
        .await?;
    assert_eq!(
        result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .map(|text| text.text.as_str()),
        Some("extra tool 4: Search")
    );

    // All three helpers are now declared.
    let names: Vec<String> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    for helper in ["search_tools", "search_prompt", "search_resource"] {
        assert!(names.contains(&helper.to_string()), "{helper} must be declared");
    }

    client.cancel().await?;
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_search_collision_suppresses_helper_but_keeps_business_tool() -> TestResult {
    run_test_with_timeout(
        "gateway_search_collision_suppresses_helper_but_keeps_business_tool",
        gateway_search_collision_suppresses_helper_but_keeps_business_tool_inner(),
    )
    .await
}

/// A real upstream tool named search_tools owns the name: the helper stays
/// suppressed (recorded as a projection issue), the business tool keeps
/// routing upstream, and the other helpers keep working.
async fn gateway_search_collision_suppresses_helper_but_keeps_business_tool_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    assert_success(
        &run_cli(
            &fixture_add_args(
                &config_path,
                "demo",
                &[
                    ("MCPSTORE_FIXTURE_EXTRA_TOOLS", "8".to_string()),
                    ("MCPSTORE_FIXTURE_COLLISION", "1".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_PROMPTS", "9".to_string()),
                    ("MCPSTORE_FIXTURE_EXTRA_RESOURCES", "5".to_string()),
                    ("MCPSTORE_FIXTURE_TEMPLATES", "4".to_string()),
                ],
            ),
        ),
        "add demo",
    );

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
                "collision gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };

    assert_eq!(client.list_all_prompts().await?.len(), 10);
    assert_eq!(client.list_all_resources().await?.len(), 6);
    assert_eq!(client.list_all_resource_templates().await?.len(), 4);

    let names: Vec<String> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(
        names.iter().filter(|name| *name == "search_tools").count(),
        1,
        "exactly one search_tools may be declared (the business tool)"
    );
    assert!(names.contains(&"search_prompt".to_string()));
    assert!(names.contains(&"search_resource".to_string()));

    // The business tool keeps its name and routes upstream.
    let result = client
        .call_tool(
            CallToolRequestParams::new("search_tools")
                .with_arguments(call_args(serde_json::json!({"query": "mine"}))),
        )
        .await?;
    assert_eq!(
        result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .map(|text| text.text.as_str()),
        Some("upstream search_tools: mine")
    );

    // The suppression is visible as a projection issue on the mount.
    let projection: serde_json::Value = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/projection", gateway_child.port))
        .send()
        .await?
        .json()
        .await?;
    let suppressed = serde_json::to_string(&projection)?;
    assert!(
        suppressed.contains("SearchHelperSuppressed"),
        "projection must record the suppressed helper: {suppressed}"
    );

    // The other two helpers still search their own kinds.
    let hit = client
        .call_tool(
            CallToolRequestParams::new("search_prompt").with_arguments(call_args(
                serde_json::json!({"query": "extra prompt one"}),
            )),
        )
        .await?;
    assert_eq!(
        hit.structured_content.expect("structured payload")["items"][0]["name"],
        "extra_prompt_1"
    );

    client.cancel().await?;
    let _ = gateway_child.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn gateway_search_partial_scope_never_publishes_and_mounts_stay_isolated() -> TestResult {
    run_test_with_timeout(
        "gateway_search_partial_scope_never_publishes_and_mounts_stay_isolated",
        gateway_search_partial_scope_never_publishes_and_mounts_stay_isolated_inner(),
    )
    .await
}

/// A scope page that skipped a failed instance is partial and never publishes
/// as the catalog — no helper may appear even though 10 prompts are visible.
/// A daemon serves one gateway face at a time, so the instance mount over the
/// healthy service starts afterwards: a fresh mount has zero discovery rows
/// (nothing leaks in from the scope face) and publishes search_prompt only
/// from its own complete page.
async fn gateway_search_partial_scope_never_publishes_and_mounts_stay_isolated_inner() -> TestResult {
    let temp_dir = unique_temp_dir();
    let config_path = temp_dir.join("mcp.json");
    assert_success(
        &run_cli(
            &fixture_add_args(
                &config_path,
                "demo",
                &[("MCPSTORE_FIXTURE_EXTRA_PROMPTS", "9".to_string())],
            ),
        ),
        "add demo",
    );
    // Registered but never connectable: the scope page over it must skip it.
    let dead_args = vec![
        "add".to_string(),
        "dead".to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
        "--".to_string(),
        "definitely-not-a-real-binary-xyz".to_string(),
    ];
    assert_success(&run_cli(&dead_args), "add dead");

    let store_args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let daemon = TestDaemon::start(&store_args).await?;
    daemon.connect_service("demo", "scope=store").await?;

    // Scope mount over {demo, dead}: every prompt/resource page skips the
    // dead instance, so no discovery row may ever publish.
    let scope_child = spawn_gateway(&daemon, &["--expose-search-tools".to_string()]).await?;
    let scope_url = format!("http://127.0.0.1:{}/mcp", scope_child.port);
    let scope_client = match connect_http_client(&scope_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = scope_child.shutdown().await;
            return Err(format!(
                "scope gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };
    assert_eq!(scope_client.list_all_prompts().await?.len(), 10);
    assert_eq!(scope_client.list_all_resources().await?.len(), 1);
    assert_eq!(scope_client.list_all_resource_templates().await?.len(), 0);
    let names: Vec<String> = scope_client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names, vec!["greet".to_string()]);
    for helper in ["search_tools", "search_prompt", "search_resource"] {
        let error = scope_client
            .call_tool(
                CallToolRequestParams::new(helper)
                    .with_arguments(call_args(serde_json::json!({"query": "greet"}))),
            )
            .await;
        assert!(
            error.is_err(),
            "partial scope must never activate {helper}"
        );
    }
    scope_client.cancel().await?;
    let _ = scope_child.shutdown().await;

    // Instance mount over the same daemon, same healthy service. Cold start:
    // a fresh mount owns zero discovery rows — no helper may pre-exist.
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
    let instance_child = spawn_gateway(
        &daemon,
        &[
            "--instance-id".to_string(),
            instance_id.to_string(),
            "--expose-search-tools".to_string(),
        ],
    )
    .await?;
    let instance_url = format!("http://127.0.0.1:{}/mcp", instance_child.port);
    let instance_client = match connect_http_client(&instance_url).await {
        Ok(client) => client,
        Err(error) => {
            let stderr_output = instance_child.shutdown().await;
            return Err(format!(
                "instance gateway handshake failed: {error}; stderr:\n{stderr_output}"
            )
            .into());
        }
    };
    let cold: Vec<String> = instance_client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(cold, vec!["greet".to_string()]);

    // Its own complete prompt page publishes search_prompt; the other kinds
    // stay below threshold.
    assert_eq!(instance_client.list_all_prompts().await?.len(), 10);
    let names: Vec<String> = instance_client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert!(
        names.contains(&"search_prompt".to_string()),
        "instance mount with a complete prompt page must activate search_prompt"
    );
    assert!(!names.contains(&"search_tools".to_string()));
    assert!(!names.contains(&"search_resource".to_string()));
    let hit = instance_client
        .call_tool(
            CallToolRequestParams::new("search_prompt")
                .with_arguments(call_args(serde_json::json!({"query": "greet"}))),
        )
        .await?;
    assert_eq!(
        hit.structured_content.expect("structured payload")["items"]
            .as_array()
            .expect("items array")
            .len(),
        0,
        "prompt catalog has no greet entry; an empty hit is correct"
    );

    instance_client.cancel().await?;
    let _ = instance_child.shutdown().await;
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
    let tool_name = "greet".to_string();

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
    let tool_name = "greet".to_string();

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
        // Deadline, not iteration count: a fully parallel suite boots ~10
        // daemons + fixtures at once and daemon startup (plugin materialize,
        // KV, core bind) legitimately exceeds 5s under that load.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
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
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            let state: serde_json::Value = client
                .get(format!("{base}/state?{scope_query}"))
                .send()
                .await?
                .json()
                .await?;
            match state["data"]["phase"].as_str() {
                Some("running") => break,
                Some("failed") => return Err(format!("service failed to connect: {state}").into()),
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        // /connect is fire-and-forget: phase flips to running at
        // TransportConnected, but the instance lands in the registry only
        // after tool sync. Gateway mounts enumerate the scoped registry
        // catalog, so wait for that same view to become non-empty — polling
        // phase alone loses the race under parallel load (mount sees 0 tools).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            let catalog: serde_json::Value = client
                .get(format!("{}/tools/list?{scope_query}", self.core_url))
                .send()
                .await?
                .json()
                .await?;
            let registered = catalog["data"]["tools"]
                .as_array()
                .is_some_and(|tools| {
                    tools
                        .iter()
                        .any(|tool| tool["service_name"].as_str() == Some(service_name))
                });
            if registered {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(format!(
            "service {service_name} registered but its scope catalog stayed empty"
        )
        .into())
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline {
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
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
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

    Err(format!("streamable-http MCP handshake timed out: {last_error}").into())
}
