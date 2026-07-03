use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tokio::io::AsyncReadExt;

struct ApiProcess {
    child: tokio::process::Child,
    stdout_task: tokio::task::JoinHandle<Result<String, std::io::Error>>,
    stderr_task: tokio::task::JoinHandle<Result<String, std::io::Error>>,
}

impl ApiProcess {
    async fn stop(mut self) -> Result<(String, String), Box<dyn std::error::Error>> {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        let stdout = self.stdout_task.await??;
        let stderr = self.stderr_task.await??;
        Ok((stdout, stderr))
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
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mcpstore-api-flow-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn reserve_local_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind temp port");
    listener
        .local_addr()
        .expect("failed to read temp port")
        .port()
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

fn unique_namespace() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("mcpstore-api-flow-{nanos}")
}

async fn spawn_api_process(args: &[&str]) -> Result<ApiProcess, Box<dyn std::error::Error>> {
    let mut child = tokio::process::Command::new(cli_bin())
        .args(args)
        .current_dir(rust_root())
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout must be piped");
    let stderr = child.stderr.take().expect("stderr must be piped");
    let stdout_task = tokio::spawn(async move {
        let mut stdout = stdout;
        let mut buffer = String::new();
        stdout.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });
    let stderr_task = tokio::spawn(async move {
        let mut stderr = stderr;
        let mut buffer = String::new();
        stderr.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });

    Ok(ApiProcess {
        child,
        stdout_task,
        stderr_task,
    })
}

#[tokio::test]
#[ignore = "integration test: connect_service times out in CI/debug due to stdio transport environment variance"]
async fn api_command_serves_store_and_agent_routes_with_url_prefix(
) -> Result<(), Box<dyn std::error::Error>> {
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
        format!("PYTHONPATH={}", pythonpath),
        "--".to_string(),
        "python3".to_string(),
        fixture.display().to_string(),
    ];
    let add_stdout = assert_success(&run_cli(&add_args), "add");
    assert!(add_stdout.contains("[Success] Service added: demo"));

    let mut child = tokio::process::Command::new(cli_bin())
        .arg("api")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--url-prefix")
        .arg("/api")
        .arg("--config-path")
        .arg(config_path.display().to_string())
        .current_dir(rust_root())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout must be piped");
    let stderr = child.stderr.take().expect("stderr must be piped");
    let stdout_task = tokio::spawn(async move {
        let mut stdout = stdout;
        let mut buffer = String::new();
        stdout.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });
    let stderr_task = tokio::spawn(async move {
        let mut stderr = stderr;
        let mut buffer = String::new();
        stderr.read_to_string(&mut buffer).await?;
        Ok::<_, std::io::Error>(buffer)
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let base_url = format!("http://127.0.0.1:{port}/api");
    if let Err(error) = wait_until_ready(&client, &base_url).await {
        let _ = child.kill().await;
        let _ = child.wait().await;
        let stdout_output = stdout_task.await??;
        let stderr_output = stderr_task.await??;
        return Err(format!(
            "Rust API 服务启动失败: {error}\nstdout=\n{stdout_output}\nstderr=\n{stderr_output}"
        )
        .into());
    }

    let health = client.get(format!("{base_url}/health")).send().await?;
    assert!(health.status().is_success());
    let health_payload = health.json::<Value>().await?;
    assert_eq!(health_payload["status"], "ok");

    let services = client
        .get(format!("{base_url}/for_store/list_services"))
        .send()
        .await?;
    assert!(services.status().is_success());
    let services_payload = services.json::<Value>().await?;
    assert!(services_payload["success"].as_bool().unwrap_or(false));
    assert_eq!(services_payload["data"]["total"], 1);
    assert_eq!(services_payload["data"]["services"][0]["name"], "demo");

    let connect = client
        .post(format!("{base_url}/for_store/connect_service/demo"))
        .send()
        .await?;
    assert!(connect.status().is_success());
    let connect_payload = connect.json::<Value>().await?;
    assert!(connect_payload["success"].as_bool().unwrap_or(false));

    let tools = client
        .get(format!("{base_url}/for_store/list_tools"))
        .send()
        .await?;
    assert!(tools.status().is_success());
    let tools_payload = tools.json::<Value>().await?;
    assert_eq!(tools_payload["data"]["total"], 1);
    assert_eq!(tools_payload["data"]["tools"][0]["name"], "demo_greet");

    let resources = client
        .get(format!("{base_url}/for_store/list_resources"))
        .send()
        .await?;
    assert!(resources.status().is_success());
    let resources_payload = resources.json::<Value>().await?;
    assert_eq!(resources_payload["data"]["total"], 1);
    assert_eq!(
        resources_payload["data"]["resources"][0]["uri"],
        "fixture://docs/readme"
    );

    let resource_templates = client
        .get(format!("{base_url}/for_store/list_resource_templates"))
        .send()
        .await?;
    assert!(resource_templates.status().is_success());
    let resource_templates_payload = resource_templates.json::<Value>().await?;
    assert_eq!(resource_templates_payload["data"]["total"], 0);

    let read_resource = client
        .get(format!("{base_url}/for_store/read_resource"))
        .query(&[("uri", "fixture://docs/readme")])
        .send()
        .await?;
    assert!(read_resource.status().is_success());
    let read_resource_payload = read_resource.json::<Value>().await?;
    assert_eq!(
        read_resource_payload["data"]["contents"][0]["text"],
        "This is the MCPStore fixture resource."
    );

    let prompts = client
        .get(format!("{base_url}/for_store/list_prompts"))
        .send()
        .await?;
    assert!(prompts.status().is_success());
    let prompts_payload = prompts.json::<Value>().await?;
    assert_eq!(prompts_payload["data"]["total"], 1);
    assert_eq!(
        prompts_payload["data"]["prompts"][0]["name"],
        "demo_explain"
    );

    let get_prompt = client
        .post(format!("{base_url}/for_store/get_prompt"))
        .json(&json!({
            "prompt_name": "demo_explain",
            "args": {"topic": "resources"},
        }))
        .send()
        .await?;
    assert!(get_prompt.status().is_success());
    let get_prompt_payload = get_prompt.json::<Value>().await?;
    assert_eq!(
        get_prompt_payload["data"]["messages"][0]["content"]["text"],
        "Explain resources via fixture prompt."
    );

    let call = client
        .post(format!("{base_url}/for_store/call_tool"))
        .json(&json!({
            "tool_name": "demo_greet",
            "args": {"name": "API"},
        }))
        .send()
        .await?;
    assert!(call.status().is_success());
    let call_payload = call.json::<Value>().await?;
    assert_eq!(call_payload["data"]["content"][0]["text"], "Hello, API!");

    let session_create = client
        .post(format!("{base_url}/sessions/create"))
        .json(&json!({
            "session_id": "api-session",
            "lease_seconds": 60,
            "metadata": {"owner": "api-flow"},
        }))
        .send()
        .await?;
    assert!(session_create.status().is_success());
    let session_create_payload = session_create.json::<Value>().await?;
    let session_key = session_create_payload["data"]["session"]["session_key"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(session_key, "store:global:api-session");

    let session_bind = client
        .post(format!("{base_url}/sessions/bind_service/{session_key}"))
        .json(&json!({"service_name": "demo"}))
        .send()
        .await?;
    assert!(session_bind.status().is_success());

    let session_tools = client
        .get(format!("{base_url}/sessions/list_tools/{session_key}"))
        .send()
        .await?;
    assert!(session_tools.status().is_success());
    let session_tools_payload = session_tools.json::<Value>().await?;
    assert_eq!(session_tools_payload["data"]["total"], 1);
    assert_eq!(
        session_tools_payload["data"]["tools"][0]["name"],
        "demo_greet"
    );

    let session_call = client
        .post(format!("{base_url}/sessions/call_tool/{session_key}"))
        .json(&json!({
            "tool_name": "demo_greet",
            "args": {"name": "Session API"},
        }))
        .send()
        .await?;
    assert!(session_call.status().is_success());
    let session_call_payload = session_call.json::<Value>().await?;
    assert_eq!(
        session_call_payload["data"]["content"][0]["text"],
        "Hello, Session API!"
    );

    let session_list = client
        .get(format!("{base_url}/sessions/list"))
        .query(&[("scope", "store")])
        .send()
        .await?;
    assert!(session_list.status().is_success());
    let session_list_payload = session_list.json::<Value>().await?;
    assert_eq!(session_list_payload["data"]["total"], 1);

    let session_close = client
        .post(format!("{base_url}/sessions/close/{session_key}"))
        .json(&json!({"reason": "done"}))
        .send()
        .await?;
    assert!(session_close.status().is_success());
    let session_close_payload = session_close.json::<Value>().await?;
    assert_eq!(session_close_payload["data"]["status"]["status"], "closed");

    let assign = client
        .post(format!("{base_url}/for_agent/agent-a/assign_service/demo"))
        .send()
        .await?;
    assert!(assign.status().is_success());
    let assign_payload = assign.json::<Value>().await?;
    assert!(assign_payload["success"].as_bool().unwrap_or(false));

    let agent_services = client
        .get(format!("{base_url}/for_agent/agent-a/list_services"))
        .send()
        .await?;
    assert!(agent_services.status().is_success());
    let agent_services_payload = agent_services.json::<Value>().await?;
    assert_eq!(agent_services_payload["data"]["total"], 1);
    assert_eq!(
        agent_services_payload["data"]["services"][0]["name"],
        "demo"
    );

    let agent_tools = client
        .get(format!("{base_url}/for_agent/agent-a/list_tools"))
        .send()
        .await?;
    assert!(agent_tools.status().is_success());
    let agent_tools_payload = agent_tools.json::<Value>().await?;
    assert_eq!(agent_tools_payload["data"]["total"], 1);
    assert_eq!(
        agent_tools_payload["data"]["tools"][0]["name"],
        "demo_greet"
    );

    let agent_prompts = client
        .get(format!("{base_url}/for_agent/agent-a/list_prompts"))
        .send()
        .await?;
    assert!(agent_prompts.status().is_success());
    let agent_prompts_payload = agent_prompts.json::<Value>().await?;
    assert_eq!(agent_prompts_payload["data"]["total"], 1);
    assert_eq!(
        agent_prompts_payload["data"]["prompts"][0]["name"],
        "demo_explain"
    );

    let cache_health = client
        .get(format!("{base_url}/cache/health"))
        .send()
        .await?;
    assert!(cache_health.status().is_success());
    let cache_health_payload = cache_health.json::<Value>().await?;
    assert!(cache_health_payload["success"].as_bool().unwrap_or(false));

    let _ = child.kill().await;
    let _ = child.wait().await;
    let _ = stdout_task.await??;
    let _ = stderr_task.await??;
    Ok(())
}

async fn wait_until_ready(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_error = String::new();

    for _ in 0..50 {
        match client.get(format!("{base_url}/health")).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                last_error = format!("health status={}", response.status());
            }
            Err(error) => {
                last_error = error.to_string();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    Err(format!("等待 Rust API 就绪超时: {last_error}").into())
}

#[tokio::test]
async fn api_processes_share_session_state_through_redis_backend(
) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping redis integration test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };

    let namespace = unique_namespace();
    let port_a = reserve_local_port();
    let port_b = reserve_local_port();

    let mut api_a = Some(
        spawn_api_process(&[
            "api",
            "--host",
            "127.0.0.1",
            "--port",
            &port_a.to_string(),
            "--source",
            "db",
            "--redis-url",
            &redis_url,
            "--namespace",
            &namespace,
        ])
        .await?,
    );
    let mut api_b = Some(
        spawn_api_process(&[
            "api",
            "--host",
            "127.0.0.1",
            "--port",
            &port_b.to_string(),
            "--source",
            "db",
            "--redis-url",
            &redis_url,
            "--namespace",
            &namespace,
        ])
        .await?,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let base_a = format!("http://127.0.0.1:{port_a}");
    let base_b = format!("http://127.0.0.1:{port_b}");

    if let Err(error) = wait_until_ready(&client, &base_a).await {
        let (stdout, stderr) = api_a.take().unwrap().stop().await?;
        let _ = api_b.take().unwrap().stop().await?;
        return Err(format!(
            "Rust API A 服务启动失败: {error}\nstdout=\n{stdout}\nstderr=\n{stderr}"
        )
        .into());
    }
    if let Err(error) = wait_until_ready(&client, &base_b).await {
        let _ = api_a.take().unwrap().stop().await?;
        let (stdout, stderr) = api_b.take().unwrap().stop().await?;
        return Err(format!(
            "Rust API B 服务启动失败: {error}\nstdout=\n{stdout}\nstderr=\n{stderr}"
        )
        .into());
    }

    let health_a = client.get(format!("{base_a}/health")).send().await?;
    assert!(health_a.status().is_success());
    let health_a_payload = health_a.json::<Value>().await?;
    assert_eq!(health_a_payload["backend"], "redis");

    let create = client
        .post(format!("{base_a}/sessions/create"))
        .json(&json!({
            "session_id": "redis-api-session",
            "lease_seconds": 60,
            "metadata": {"owner": "api-redis-flow"},
        }))
        .send()
        .await?;
    assert!(create.status().is_success());
    let create_payload = create.json::<Value>().await?;
    let session_key = create_payload["data"]["session"]["session_key"]
        .as_str()
        .unwrap()
        .to_string();

    let set_state = client
        .post(format!("{base_a}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "cursor",
            "value": {"page": 7},
        }))
        .send()
        .await?;
    assert!(set_state.status().is_success());

    let read_from_b = client
        .get(format!("{base_b}/sessions/state/value"))
        .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
        .send()
        .await?;
    assert!(read_from_b.status().is_success());
    let read_from_b_payload = read_from_b.json::<Value>().await?;
    assert_eq!(read_from_b_payload["data"]["value"]["page"], 7);

    let close_from_b = client
        .post(format!("{base_b}/sessions/close"))
        .json(&json!({"session_key": session_key, "reason": "closed-by-peer"}))
        .send()
        .await?;
    assert!(close_from_b.status().is_success());

    let write_after_close_from_a = client
        .post(format!("{base_a}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "after_close",
            "value": true,
        }))
        .send()
        .await?;
    assert_eq!(
        write_after_close_from_a.status(),
        axum::http::StatusCode::CONFLICT
    );
    let write_after_close_payload = write_after_close_from_a.json::<Value>().await?;
    assert_eq!(
        write_after_close_payload["errors"][0]["code"],
        "SESSION_NOT_ACTIVE"
    );

    let _ = api_a.take().unwrap().stop().await?;
    let _ = api_b.take().unwrap().stop().await?;
    Ok(())
}

#[tokio::test]
async fn api_cache_switch_migrates_session_state_to_shared_redis_backend(
) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping redis integration test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };

    let namespace = unique_namespace();
    let port_a = reserve_local_port();
    let port_b = reserve_local_port();

    let mut api_a = Some(
        spawn_api_process(&[
            "api",
            "--host",
            "127.0.0.1",
            "--port",
            &port_a.to_string(),
            "--source",
            "db",
            "--backend",
            "memory",
        ])
        .await?,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let base_a = format!("http://127.0.0.1:{port_a}");

    if let Err(error) = wait_until_ready(&client, &base_a).await {
        let (stdout, stderr) = api_a.take().unwrap().stop().await?;
        return Err(format!(
            "Rust API A 服务启动失败: {error}\nstdout=\n{stdout}\nstderr=\n{stderr}"
        )
        .into());
    }

    let health_a = client.get(format!("{base_a}/health")).send().await?;
    assert!(health_a.status().is_success());
    let health_a_payload = health_a.json::<Value>().await?;
    assert_eq!(health_a_payload["backend"], "memory");

    let create = client
        .post(format!("{base_a}/sessions/create"))
        .json(&json!({
            "session_id": "migrated-api-session",
            "lease_seconds": 90,
            "metadata": {"owner": "api-cache-switch-flow"},
        }))
        .send()
        .await?;
    assert!(create.status().is_success());
    let create_payload = create.json::<Value>().await?;
    let session_key = create_payload["data"]["session"]["session_key"]
        .as_str()
        .unwrap()
        .to_string();

    let set_state = client
        .post(format!("{base_a}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "cursor",
            "value": {"page": 11},
        }))
        .send()
        .await?;
    assert!(set_state.status().is_success());

    let switch = client
        .post(format!("{base_a}/cache/switch"))
        .json(&json!({
            "backend": "redis",
            "redis_url": redis_url,
            "namespace": namespace,
        }))
        .send()
        .await?;
    assert!(switch.status().is_success());
    let switch_payload = switch.json::<Value>().await?;
    assert!(switch_payload["success"].as_bool().unwrap_or(false));

    let health_after_switch = client.get(format!("{base_a}/health")).send().await?;
    assert!(health_after_switch.status().is_success());
    let health_after_switch_payload = health_after_switch.json::<Value>().await?;
    assert_eq!(health_after_switch_payload["backend"], "redis");

    let mut api_b = Some(
        spawn_api_process(&[
            "api",
            "--host",
            "127.0.0.1",
            "--port",
            &port_b.to_string(),
            "--source",
            "db",
            "--redis-url",
            &redis_url,
            "--namespace",
            &namespace,
        ])
        .await?,
    );
    let base_b = format!("http://127.0.0.1:{port_b}");
    if let Err(error) = wait_until_ready(&client, &base_b).await {
        let _ = api_a.take().unwrap().stop().await?;
        let (stdout, stderr) = api_b.take().unwrap().stop().await?;
        return Err(format!(
            "Rust API B 服务启动失败: {error}\nstdout=\n{stdout}\nstderr=\n{stderr}"
        )
        .into());
    }

    let read_from_b = client
        .get(format!("{base_b}/sessions/state/value"))
        .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
        .send()
        .await?;
    assert!(read_from_b.status().is_success());
    let read_from_b_payload = read_from_b.json::<Value>().await?;
    assert_eq!(read_from_b_payload["data"]["value"]["page"], 11);

    let _ = api_a.take().unwrap().stop().await?;
    let _ = api_b.take().unwrap().stop().await?;
    Ok(())
}
