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
    let fixture = HostFixture::start().await?;
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
async fn request_round_trip_and_deadline_error() -> TestResult<()> {
    let _guard = HOST_TEST_LOCK.lock().unwrap();
    let fixture = HostFixture::start().await?;
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
    let fixture = HostFixture::start().await?;
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

struct HostFixture {
    child: tokio::process::Child,
    socket: PathBuf,
    dir: PathBuf,
    instance_id: String,
}

impl HostFixture {
    async fn start() -> TestResult<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mcpstore-kernel-host-{nanos}"));
        std::fs::create_dir_all(&dir)?;
        let socket = dir.join("kernel.sock");
        let pid = dir.join("kernel.pid");
        let config_path = dir.join("config.json");
        let service_name = "execution-kernel-host";
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
        let mut child = tokio::process::Command::new(cli)
            .args([
                "start",
                "--source",
                "local",
                "--config-path",
                config_path.to_string_lossy().as_ref(),
                "--namespace",
                "test",
            ])
            .env("MCPSTORE_SOCKET", &socket)
            .env("MCPSTORE_PID", &pid)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        for _ in 0..100 {
            if socket.exists() {
                let instance_id =
                    mcpstore::ServiceInstanceKey::new(service_name, mcpstore::ScopeRef::Store)
                        .instance_id()
                        .to_string();
                return Ok(Self {
                    child,
                    socket,
                    dir,
                    instance_id,
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
        return Err("KernelHost socket did not appear".into());
    }

    async fn stop(mut self) -> TestResult<()> {
        self.child.start_kill()?;
        let _ = self.child.wait().await;
        std::fs::remove_dir_all(&self.dir).ok();
        Ok(())
    }
}
