use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

fn fixture_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/execution_mcp_server.py")
        .canonicalize()
        .expect("failed to resolve execution fixture")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
fn service_instance_id(name: &str) -> String {
    mcpstore::ServiceInstanceKey::new(name, mcpstore::ScopeRef::Store)
        .instance_id()
        .to_string()
}

async fn write_line<W>(writer: &mut W, value: &serde_json::Value) -> TestResult<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer
        .write_all(format!("{}\n", serde_json::to_string(value)?).as_bytes())
        .await?;
    Ok(())
}

async fn read_line<R>(reader: &mut R) -> TestResult<serde_json::Value>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        return Err("KernelHost closed connection".into());
    }
    Ok(serde_json::from_str(line.trim())?)
}

type TestResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
static HOST_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn handshake_rejects_wrong_namespace() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let mut fixture = HostFixture::start().await?;
    fixture.instance_id = service_instance_id("execution-kernel-host");
    let stream = UnixStream::connect(&fixture.socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    write_line(
        &mut writer,
        &serde_json::json!({
            "protocol_version": 1,
            "namespace": "other",
            "client_capabilities": []
        }),
    )
    .await?;
    let response = read_line(&mut reader).await?;
    assert!(response["error"]["code"].is_string());
    assert_eq!(response["error"]["code"], "connection_scope");
    fixture.stop().await
}

#[tokio::test]
async fn remote_daemon_endpoint_requires_token() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let port = 18000 + (nanos % 2000) as u16;
    let fixture = HostFixture::start_remote(port, "secret").await?;
    let ok = run_cli(&[
        "list".into(),
        "--output".into(),
        "json".into(),
        "--daemon-endpoint".into(),
        format!("127.0.0.1:{port}"),
        "--daemon-namespace".into(),
        "mcpstore".into(),
        "--daemon-token".into(),
        "secret".into(),
    ])?;
    assert!(ok.status.success(), "remote list failed: {ok:?}");
    let bad = run_cli(&[
        "list".into(),
        "--daemon-endpoint".into(),
        format!("127.0.0.1:{port}"),
        "--daemon-namespace".into(),
        "mcpstore".into(),
        "--daemon-token".into(),
        "wrong".into(),
    ])?;
    assert!(!bad.status.success(), "wrong token was accepted: {bad:?}");
    assert!(String::from_utf8_lossy(&bad.stderr).contains("connection_auth"));
    fixture.stop().await
}

#[tokio::test]
async fn request_round_trip_and_deadline_error() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let mut fixture = HostFixture::start().await?;
    fixture.instance_id = service_instance_id("execution-kernel-host");
    let stream = UnixStream::connect(&fixture.socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    write_line(
        &mut writer,
        &serde_json::json!({"protocol_version": 1, "namespace": "test", "client_capabilities": []}),
    )
    .await?;
    let handshake = read_line(&mut reader).await?;
    assert!(handshake["error"].is_null());

    write_line(
        &mut writer,
        &serde_json::json!({
            "request_id": 7,
            "operation": "WaitService",
            "payload": {"instance_id": "00000000-0000-5000-8000-000000000000", "timeout": 1},
            "deadline_ms": 1
        }),
    )
    .await?;
    let response = read_line(&mut reader).await?;
    assert_eq!(response["request_id"], 7);
    assert!(!response["error"].is_null());

    write_line(
        &mut writer,
        &serde_json::json!({
            "request_id": 8,
            "operation": "ShowConfig",
            "payload": {},
            "deadline_ms": 1000
        }),
    )
    .await?;
    let response = read_line(&mut reader).await?;
    assert_eq!(response["request_id"], 8);
    assert!(response["error"].is_null());

    std::fs::remove_file(&fixture.socket)?;
    write_line(
        &mut writer,
        &serde_json::json!({
            "request_id": 9,
            "operation": "ShowConfig",
            "payload": {},
            "deadline_ms": 1000
        }),
    )
    .await?;
    let response = read_line(&mut reader).await?;
    assert_eq!(response["request_id"], 9);
    assert!(response["error"].is_null());
    fixture.stop().await
}

#[tokio::test]
async fn stream_execution_emits_started_progress_and_finished() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let mut fixture = HostFixture::start().await?;
    fixture.instance_id = service_instance_id("execution-kernel-host");
    let stream = UnixStream::connect(&fixture.socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    write_line(
        &mut writer,
        &serde_json::json!({"protocol_version": 1, "namespace": "test", "client_capabilities": ["requests", "streams"]}),
    )
    .await?;
    let handshake = read_line(&mut reader).await?;
    assert!(handshake["error"].is_null());

    write_line(
        &mut writer,
        &serde_json::json!({
            "request_id": 9,
            "operation": "StreamToolExecution",
            "payload": {"instance_id": fixture.instance_id, "tool_name": "progress", "args": {}, "connect": true},
            "deadline_ms": 10000
        }),
    )
    .await?;
    let started = read_line(&mut reader).await?;
    assert!(started["event"]["Started"].is_object(), "{started}");
    let mut saw_progress = false;
    let finished = loop {
        let response = read_line(&mut reader).await?;
        if let Some(progress) = response["event"]["Progress"].as_object() {
            saw_progress = true;
            assert_eq!(progress["message"], "fixture-progress");
            continue;
        }
        if response["event"]["Finished"].is_object() || response["error"].is_object() {
            break response;
        }
        eprintln!("unexpected KernelHost response: {response}");
    };
    assert!(saw_progress, "stream must emit progress events");
    let result = finished["event"]["Finished"]["result"]
        .get("content")
        .or_else(|| finished["event"]["Finished"].get("content"))
        .unwrap_or(&serde_json::Value::Null);
    assert_eq!(result[0]["text"], "fixture-complete", "{finished}");
    fixture.stop().await
}

#[tokio::test]
async fn redis_dataplane_queue_is_consumed_by_control_plane_daemon() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping redis integration test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let namespace = format!("mcpstore-dataplane-redis-{nanos}");
    let fixture = HostFixture::start_redis(RedisHostSource {
        url: redis_url.clone(),
        namespace: namespace.clone(),
    })
    .await?;
    let result = dataplane_queue_consumed_by_daemon(fixture, redis_url, namespace).await;
    result
}

async fn dataplane_queue_consumed_by_daemon(
    fixture: HostFixture,
    redis_url: String,
    namespace: String,
) -> TestResult<()> {
    let source_args = [
        "--source".to_string(),
        "db".to_string(),
        "--store".to_string(),
        "redis".to_string(),
        "--store-config".to_string(),
        format!(r#"{{"url":"{redis_url}"}}"#),
        "--namespace".to_string(),
        namespace,
        "--node-mode".to_string(),
        "data".to_string(),
    ];
    let mut add_args = vec![
        "add".to_string(),
        "shared-service".to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
    ];
    add_args.extend(source_args.iter().cloned());
    add_args.extend([
        "--".to_string(),
        "python3".to_string(),
        fixture_script().display().to_string(),
    ]);
    let add = run_cli(&add_args)?;
    assert!(add.status.success(), "data-plane add failed: {add:?}");
    let request_id = queued_request_id(&add)?;
    wait_for_applied(&request_id, &source_args)?;

    let mut connect_args = vec![
        "connect".to_string(),
        "shared-service".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    connect_args.extend(source_args.clone());
    let connect = run_cli(&connect_args)?;
    assert!(
        connect.status.success(),
        "data-plane connect failed: {connect:?}"
    );
    let connect: serde_json::Value = serde_json::from_slice(&connect.stdout)?;
    let connect_request_id = connect["request_id"]
        .as_str()
        .ok_or("connect did not return request_id")?
        .to_string();

    wait_for_applied(&connect_request_id, &source_args)?;

    let mut list_args = vec![
        "list".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    list_args.extend(source_args.iter().take(8).cloned());
    let list = run_cli(&list_args)?;
    assert!(list.status.success(), "data-plane list failed: {list:?}");
    let list: serde_json::Value = serde_json::from_slice(&list.stdout)?;
    assert_eq!(list["total"], 1, "{list}");
    assert_eq!(
        list["services"][0]["readiness"], "ready",
        "control-plane daemon must apply queued connect: {list}"
    );
    assert_eq!(list["services"][0]["tools_count"], 10, "{list}");

    fixture.stop().await
}

fn wait_for_applied(request_id: &str, source_args: &[String]) -> TestResult<()> {
    let mut args = vec![
        "request".to_string(),
        "wait".to_string(),
        request_id.to_string(),
        "--output".to_string(),
        "json".to_string(),
        "--timeout".to_string(),
        "10".to_string(),
    ];
    args.extend(source_args.iter().take(8).cloned());
    let output = run_cli(&args)?;
    assert!(output.status.success(), "request wait failed: {output:?}");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(response["status"], "applied", "{response}");
    Ok(())
}

fn queued_request_id(output: &std::process::Output) -> TestResult<String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with("[Queued] "))
        .ok_or_else(|| format!("add did not return queue receipt: {stdout}"))?;
    line.rsplit("request_id=")
        .next()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("queue receipt has no request_id: {line}"))
        .map_err(Into::into)
}

#[tokio::test]
async fn host_and_cli_share_kernel_authority_through_redis_backend() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        eprintln!("skipping redis integration test: MCPSTORE_TEST_REDIS_URL is not set");
        return Ok(());
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let namespace = format!("mcpstore-kernel-consistency-{nanos}");
    let fixture = HostFixture::start_redis(RedisHostSource {
        url: redis_url.clone(),
        namespace: namespace.clone(),
    })
    .await?;
    let result = host_and_cli_share_kernel_authority(fixture, redis_url, namespace).await;
    result
}

async fn host_and_cli_share_kernel_authority(
    fixture: HostFixture,
    redis_url: String,
    namespace: String,
) -> TestResult<()> {
    let source_args = [
        "--source".to_string(),
        "db".to_string(),
        "--store".to_string(),
        "redis".to_string(),
        "--store-config".to_string(),
        format!(r#"{{"url":"{redis_url}"}}"#),
        "--namespace".to_string(),
        namespace.clone(),
    ];
    let service_name = "shared-service";
    let mut add_args = vec![
        "add".to_string(),
        service_name.to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
    ];
    add_args.extend(source_args.clone());
    add_args.extend([
        "--".to_string(),
        "python3".to_string(),
        fixture_script().display().to_string(),
    ]);
    let add = run_cli(&add_args)?;
    assert!(add.status.success(), "shared add failed: {add:?}");

    let mut store_list_args = vec![
        "list".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    store_list_args.extend(source_args.clone());
    let store_list = run_cli(&store_list_args)?;
    assert!(
        store_list.status.success(),
        "store list failed: {store_list:?}"
    );
    let store_list: serde_json::Value = serde_json::from_slice(&store_list.stdout)?;
    assert_eq!(store_list["total"], 1);
    assert_eq!(store_list["services"][0]["service_name"], service_name);

    let mut host = connect_host(&fixture.socket, &namespace).await?;
    let declare = host
        .request(
            "DeclareServiceScope",
            &serde_json::json!({
                "service_name": service_name,
                "scope": {"type": "agent", "agent_id": "agent-1"},
                "descriptor": {},
            }),
        )
        .await?;
    assert!(declare["error"].is_null(), "{declare}");
    let host_agent_list = host
        .request(
            "ListServices",
            &serde_json::json!({"scope": {"type": "agent", "agent_id": "agent-1"}}),
        )
        .await?;
    assert_eq!(host_agent_list["result"]["total"], 1, "{host_agent_list}");

    let mut agent_list_args = vec![
        "list".to_string(),
        "--scope".to_string(),
        "agent".to_string(),
        "--agent".to_string(),
        "agent-1".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    agent_list_args.extend(source_args.clone());
    let agent_list = run_cli(&agent_list_args)?;
    assert!(
        agent_list.status.success(),
        "agent list failed: {agent_list:?}"
    );
    let agent_list: serde_json::Value = serde_json::from_slice(&agent_list.stdout)?;
    assert_eq!(agent_list["total"], 1);
    assert_eq!(agent_list["services"][0]["service_name"], service_name);

    let instance_id = service_instance_id(service_name);
    let mut subscriber = connect_host(&fixture.socket, &namespace).await?;
    subscriber
        .request_without_response("SubscribeEvents", &serde_json::json!({}))
        .await?;
    let connect = host
        .request(
            "ConnectService",
            &serde_json::json!({"instance_id": instance_id}),
        )
        .await?;
    assert!(connect["error"].is_null(), "{connect}");
    let event = subscriber.next_event().await?;
    assert_eq!(event["event_type"], "SERVICE_STATE_CHANGED", "{event}");

    let host_auth = host
        .request(
            "AuthStatus",
            &serde_json::json!({"instance_id": instance_id}),
        )
        .await?;
    assert!(host_auth["error"].is_null(), "{host_auth}");
    let mut auth_args = vec![
        "auth".to_string(),
        "status".to_string(),
        instance_id.clone(),
        "--output".to_string(),
        "json".to_string(),
    ];
    auth_args.extend(source_args);
    let auth = run_cli(&auth_args)?;
    assert!(auth.status.success(), "auth status failed: {auth:?}");
    let auth: serde_json::Value = serde_json::from_slice(&auth.stdout)?;
    assert_eq!(auth["auth"], host_auth["result"]["auth"]);

    let disconnect = host
        .request(
            "DisconnectService",
            &serde_json::json!({"instance_id": instance_id}),
        )
        .await?;
    assert!(disconnect["error"].is_null(), "{disconnect}");

    fixture.stop().await
}

fn run_cli(args: &[String]) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    std::process::Command::new(repo_root().join("target/debug/mcpstore"))
        .args(args)
        .output()
        .map_err(Into::into)
}

struct HostConnection {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next_request_id: u64,
}

impl HostConnection {
    async fn request_without_response(
        &mut self,
        operation: &str,
        payload: &serde_json::Value,
    ) -> TestResult<()> {
        self.write_request(operation, payload).await
    }

    async fn request(
        &mut self,
        operation: &str,
        payload: &serde_json::Value,
    ) -> TestResult<serde_json::Value> {
        self.write_request(operation, payload).await?;
        self.next_response().await
    }

    async fn write_request(
        &mut self,
        operation: &str,
        payload: &serde_json::Value,
    ) -> TestResult<()> {
        self.next_request_id += 1;
        write_line(
            &mut self.writer,
            &serde_json::json!({
                "request_id": self.next_request_id,
                "operation": operation,
                "payload": payload,
                "deadline_ms": 10000,
            }),
        )
        .await
    }

    async fn next_response(&mut self) -> TestResult<serde_json::Value> {
        read_line(&mut self.reader).await
    }

    async fn next_event(&mut self) -> TestResult<serde_json::Value> {
        let response = tokio::time::timeout(Duration::from_secs(5), self.next_response())
            .await
            .map_err(|_| "KernelHost event timeout")??;
        let event = response["event"]["Finished"].clone();
        if event.is_null() {
            return Err(format!("KernelHost returned non-event response: {response}").into());
        }
        Ok(event)
    }
}

async fn connect_host(socket: &Path, namespace: &str) -> TestResult<HostConnection> {
    let stream = UnixStream::connect(socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    write_line(
        &mut writer,
        &serde_json::json!({
            "protocol_version": 1,
            "namespace": namespace,
            "client_capabilities": ["requests", "events"],
        }),
    )
    .await?;
    let handshake = read_line(&mut reader).await?;
    assert!(handshake["error"].is_null(), "{handshake}");
    Ok(HostConnection {
        reader,
        writer,
        next_request_id: 0,
    })
}

struct HostFixture {
    child: tokio::process::Child,
    socket: PathBuf,
    dir: PathBuf,
    instance_id: String,
}

struct RedisHostSource {
    url: String,
    namespace: String,
}

impl HostFixture {
    async fn start() -> TestResult<Self> {
        Self::start_local("test").await
    }

    async fn start_remote(port: u16, token: &str) -> TestResult<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mcpstore-kernel-host-{nanos}"));
        std::fs::create_dir_all(&dir)?;
        let socket = dir.join("kernel.sock");
        let pid = dir.join("kernel.pid");
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, b"{}")?;
        std::fs::write(
            dir.join("config.toml"),
            format!("[server]\ncore_enabled = false\napp_enabled = false\nweb_enabled = false\nkernel_port = {port}\nkernel_token = \"{token}\"\n"),
        )?;
        let child = tokio::process::Command::new(repo_root().join("target/debug/mcpstore"))
            .args(["start", "--source", "local", "--config-path"])
            .arg(&config_path)
            .arg("--namespace")
            .arg("mcpstore")
            .env("MCPSTORE_SOCKET", &socket)
            .env("MCPSTORE_PID", &pid)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Self::wait_for_socket(child, socket, dir).await
    }

    async fn start_redis(source: RedisHostSource) -> TestResult<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mcpstore-kernel-host-{nanos}"));
        std::fs::create_dir_all(&dir)?;
        let socket = dir.join("kernel.sock");
        let pid = dir.join("kernel.pid");
        let cli = repo_root().join("target/debug/mcpstore");
        // 本契约只测 kernel socket；禁用 HTTP 面，避免端口冲突
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, b"{}")?;
        std::fs::write(
            dir.join("config.toml"),
            "[server]\ncore_enabled = false\napp_enabled = false\nweb_enabled = false\n",
        )?;
        let child = tokio::process::Command::new(cli)
            .args([
                "start",
                "--source",
                "db",
                "--config-path",
                config_path.to_string_lossy().as_ref(),
                "--store",
                "redis",
                "--store-config",
                &format!(r#"{{"url":"{}"}}"#, source.url),
                "--namespace",
                &source.namespace,
            ])
            .env("MCPSTORE_SOCKET", &socket)
            .env("MCPSTORE_PID", &pid)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Self::wait_for_socket(child, socket, dir).await
    }

    async fn start_local(namespace: &str) -> TestResult<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mcpstore-kernel-host-{nanos}"));
        std::fs::create_dir_all(&dir)?;
        let socket = dir.join("kernel.sock");
        let pid = dir.join("kernel.pid");
        let config_path = dir.join("config.json");
        let service_name = "execution-kernel-host";
        // 本契约只测 kernel socket；禁用 HTTP 面，避免与其他测试/本机服务抢端口
        std::fs::write(
            dir.join("config.toml"),
            "[server]\ncore_enabled = false\napp_enabled = false\nweb_enabled = false\n",
        )?;
        std::fs::write(
            &config_path,
            serde_json::to_vec(&serde_json::json!({
                "mcpServers": {
                    service_name: {
                        "command": "python3",
                        "args": [fixture_script()],
                        "transport": "stdio"
                    }
                }
            }))?,
        )?;
        let cli = repo_root().join("target/debug/mcpstore");
        let child = tokio::process::Command::new(cli)
            .args([
                "start",
                "--source",
                "local",
                "--config-path",
                config_path.to_string_lossy().as_ref(),
                "--namespace",
                namespace,
            ])
            .env("MCPSTORE_SOCKET", &socket)
            .env("MCPSTORE_PID", &pid)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Self::wait_for_socket(child, socket, dir).await
    }

    async fn wait_for_socket(
        mut child: tokio::process::Child,
        socket: PathBuf,
        dir: PathBuf,
    ) -> TestResult<Self> {
        for _ in 0..100 {
            if socket.exists() {
                return Ok(Self {
                    child,
                    socket,
                    dir,
                    instance_id: String::new(),
                });
            }
            if let Some(status) = child.try_wait()? {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut output) = child.stdout.take() {
                    tokio::io::AsyncReadExt::read_to_string(&mut output, &mut stdout)
                        .await
                        .ok();
                }
                if let Some(mut output) = child.stderr.take() {
                    tokio::io::AsyncReadExt::read_to_string(&mut output, &mut stderr)
                        .await
                        .ok();
                }
                return Err(format!(
                    "KernelHost exited early status={status} stdout={stdout} stderr={stderr}"
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Err("KernelHost socket did not appear".into())
    }

    async fn stop(mut self) -> TestResult<()> {
        self.child.start_kill()?;
        let _ = self.child.wait().await;
        std::fs::remove_dir_all(&self.dir).ok();
        Ok(())
    }
}
