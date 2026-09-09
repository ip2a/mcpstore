use clap::{Args, Subcommand};
use mcpstore::{
    client_config::{import_selected_services, inspect_client_config, ClientKind},
    config::ConfigManager,
};
use serde_json::{json, Value};

use crate::store_args::{load_kernel, StoreSourceArgs};

#[derive(Subcommand)]
pub enum ConfigAction {
    Show {
        #[arg(long)]
        path: Option<String>,
    },
    Validate {
        #[arg(long)]
        path: Option<String>,
    },
    Init {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value_t = false)]
        with_examples: bool,
    },
    Path {
        #[arg(long)]
        path: Option<String>,
    },
    AddExamples {
        #[arg(long)]
        path: Option<String>,
    },
    ImportClient {
        #[arg(long)]
        client: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        names_file: String,
        #[command(flatten)]
        store: StoreSourceArgs,
    },
}

/// daemon 运行面修改 flags（设计 §7 key 表）。修改一律经 daemon 热应用并回写 config.toml。
#[derive(Args, Debug, Default)]
pub struct ConfigEdits {
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long = "core")]
    pub core: Option<String>,
    #[arg(long = "core-port")]
    pub core_port: Option<u16>,
    #[arg(long = "app")]
    pub app: Option<String>,
    #[arg(long = "app-port")]
    pub app_port: Option<u16>,
    #[arg(long = "web")]
    pub web: Option<String>,
    #[arg(long = "web-port")]
    pub web_port: Option<u16>,
    #[arg(long = "mcp")]
    pub mcp: Option<String>,
    #[arg(long = "mcp-port")]
    pub mcp_port: Option<u16>,
    #[arg(long = "mcp-transport")]
    pub mcp_transport: Option<String>,
}

impl ConfigEdits {
    pub fn pairs(&self) -> Vec<(&'static str, Value)> {
        let mut pairs = Vec::new();
        if let Some(value) = &self.host {
            pairs.push(("host", json!(value)));
        }
        if let Some(value) = &self.core {
            pairs.push(("core", json!(value)));
        }
        if let Some(value) = self.core_port {
            pairs.push(("core-port", json!(value)));
        }
        if let Some(value) = &self.app {
            pairs.push(("app", json!(value)));
        }
        if let Some(value) = self.app_port {
            pairs.push(("app-port", json!(value)));
        }
        if let Some(value) = &self.web {
            pairs.push(("web", json!(value)));
        }
        if let Some(value) = self.web_port {
            pairs.push(("web-port", json!(value)));
        }
        if let Some(value) = &self.mcp {
            pairs.push(("mcp", json!(value)));
        }
        if let Some(value) = self.mcp_port {
            pairs.push(("mcp-port", json!(value)));
        }
        if let Some(value) = &self.mcp_transport {
            pairs.push(("mcp-transport", json!(value)));
        }
        pairs
    }
}

pub async fn run(
    action: Option<ConfigAction>,
    edits: ConfigEdits,
    json: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if let Some(action) = action {
        if !edits.pairs().is_empty() {
            return Err("config 子命令与修改 flag 不能同时使用".into());
        }
        return run_action(action).await;
    }
    if !edits.pairs().is_empty() {
        return apply_edits(edits).await;
    }
    overview(json).await
}

async fn run_action(action: ConfigAction) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::Show { path } => show(path),
        ConfigAction::Validate { path } => validate(path),
        ConfigAction::Init {
            path,
            force,
            with_examples,
        } => init(path, force, with_examples),
        ConfigAction::Path { path } => show_path(path),
        ConfigAction::AddExamples { path } => add_examples(path),
        ConfigAction::ImportClient {
            client,
            path,
            names_file,
            store,
        } => import_client(client, path, names_file, store).await,
    }
}

/// 修改面唯一入口：daemon 未运行时拒绝（不悄悄改文件）。
async fn apply_edits(edits: ConfigEdits) -> Result<(), Box<dyn std::error::Error>> {
    if !crate::daemon::protocol::is_daemon_running() {
        return Err("daemon 未运行；修改需经 daemon 热应用。先运行 mcpstore start".into());
    }
    let mut client = crate::daemon::client::connect_admin().await?;
    for (key, value) in edits.pairs() {
        client.set_daemon_config(key, value).await?;
        println!("[Success] {key} hot-applied and written to config.toml");
    }
    Ok(())
}

/// 裸 `mcpstore config`：daemon 运行面总览。
async fn overview(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(mut client) = crate::daemon::client::connect_admin().await {
        let status = client.status_host().await?;
        if json {
            println!("{status}");
            return Ok(());
        }
        println!(
            "[Success] Daemon running (pid={}, uptime={}s, namespace={})",
            status["pid"], status["uptime_s"], status["namespace"]
        );
        println!("  host: {}", host_from_status(&status));
        for listener in status["listeners"].as_array().into_iter().flatten() {
            let key = listener["key"].as_str().unwrap_or("?");
            match listener["bind"].as_str() {
                Some(bind) => println!("  {key}: http://{bind}"),
                None => println!("  {key}: off"),
            }
        }
        println!("Use this command to change settings: mcpstore config --<key> <value>");
        return Ok(());
    }

    let config = ConfigManager::new().load_app_config_or_default()?;
    if json {
        println!("{}", serde_json::to_value(&config.server)?);
        return Ok(());
    }
    println!("[Info] Daemon not running（showing config.toml values; takes effect after restart）");
    println!("  host: {}", config.server.host);
    println!(
        "  core: {} port={}",
        on_off(config.server.core_enabled),
        config.server.port
    );
    println!(
        "  app:  {} port={}",
        on_off(config.server.app_enabled),
        config.server.app_port
    );
    println!(
        "  web:  {} port={}",
        on_off(config.server.web_enabled),
        config.server.web_port
    );
    println!(
        "  mcp:  {} transport={} port={}",
        on_off(config.mcp_aggregate.enabled),
        config.mcp_aggregate.transport,
        config.mcp_aggregate.port
    );
    println!("Use this command to change settings: mcpstore config --<key> <value>");
    Ok(())
}

fn host_from_status(status: &Value) -> Value {
    let empty = json!({});
    status
        .get("listeners")
        .and_then(|listeners| listeners.as_array())
        .and_then(|listeners| listeners.first())
        .and_then(|listener| listener.get("bind"))
        .and_then(|bind| bind.as_str())
        .and_then(|bind| bind.rsplit_once(':'))
        .map(|(host, _)| json!(host))
        .unwrap_or(empty)
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}

async fn import_client(
    client: String,
    path: String,
    names_file: String,
    source: StoreSourceArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspection = inspect_client_config(parse_client(&client)?, &path)?;
    let names: Vec<String> = serde_json::from_str(&std::fs::read_to_string(names_file)?)?;
    let services = import_selected_services(&inspection, &names)?;
    let store = load_kernel(&source).await?.store().clone();
    for (name, _) in &services {
        if store.get_definition_config(name).await?.is_some() {
            return Err(format!("MCPStore service already exists: {name}").into());
        }
    }
    for (name, config) in services {
        let transport = config.infer_transport().to_string();
        store.add_service(&name, config).await?;
        println!("imported {name} (transport={transport})");
    }
    Ok(())
}

fn parse_client(value: &str) -> Result<ClientKind, Box<dyn std::error::Error>> {
    match value {
        "codex" => Ok(ClientKind::Codex),
        "claude_code" | "claude-code" => Ok(ClientKind::ClaudeCode),
        "opencode" | "open-code" => Ok(ClientKind::OpenCode),
        "cursor" => Ok(ClientKind::Cursor),
        "claude_desktop" | "claude-desktop" => Ok(ClientKind::ClaudeDesktop),
        _ => Err(format!("unsupported client: {value}").into()),
    }
}
fn mgr(path: Option<String>) -> ConfigManager {
    match path {
        Some(p) => ConfigManager::with_path(p),
        None => ConfigManager::new(),
    }
}

fn show(path: Option<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let m = mgr(path);
    let mcp_config = m.load()?;
    let app_config = m.load_app_config_or_default()?;

    println!("\n[CONFIG] Current config:");
    println!("{}", "─".repeat(50));
    println!("MCP config file: {}", m.mcp_path().display());
    println!("Global config file: {}", m.app_config_path().display());
    println!("Version: {}", app_config.version);
    println!("Description: {}", app_config.description);
    println!("Created by: {}", app_config.created_by);

    println!("\nCache:");
    println!("  Store: {}", app_config.cache.store);
    println!("  Namespace: {}", app_config.cache.namespace);
    if let Some(url) = app_config.cache.config.get("url").and_then(|v| v.as_str()) {
        println!("  Backend URL: {}", url);
    }

    println!(
        "\nMCP Services ({} configured):",
        mcp_config.mcp_servers.len()
    );

    if mcp_config.mcp_servers.is_empty() {
        println!("  No configured services");
        println!("\n[TIP] Use 'mcpstore config init --with-examples' to add example services");
    } else {
        for (name, svc) in &mcp_config.mcp_servers {
            let transport = svc.infer_transport();
            let desc = svc.description.as_deref().unwrap_or("No description");
            println!(
                "\n  [{}] {} ({} service)",
                transport.to_uppercase(),
                name,
                transport
            );
            println!("    Description: {desc}");
            if let Some(ref url) = svc.url {
                println!("    URL: {url}");
            }
            if let Some(ref cmd) = svc.command {
                println!("    Command: {cmd}");
                if !svc.args.is_empty() {
                    println!("    Args: {}", svc.args.join(" "));
                }
            }
        }
    }
    Ok(())
}

fn validate(path: Option<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let m = mgr(path);
    match m
        .validate()
        .and_then(|_| m.load_app_config_or_default().map(|_| ()))
    {
        Ok(()) => {
            println!("[Success] Config validation passed");
            Ok(())
        }
        Err(e) => {
            eprintln!("[Error] {e}");
            Err(e.into())
        }
    }
}

fn init(
    path: Option<String>,
    force: bool,
    with_examples: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let m = mgr(path);
    if (m.exists() || m.app_config_exists()) && !force {
        eprintln!(
            "[Warning] Config file already exists: mcp={} config={}",
            m.mcp_path().display(),
            m.app_config_path().display()
        );
        eprintln!("Use --force to overwrite");
        return Ok(());
    }
    m.init(with_examples, None)?;
    println!(
        "[Success] MCP config initialized: {}",
        m.mcp_path().display()
    );
    println!(
        "[Success] Global config initialized: {}",
        m.app_config_path().display()
    );
    if with_examples {
        println!("[TIP] Example services added, edit files to customize");
    }
    Ok(())
}

fn show_path(path: Option<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let m = mgr(path);
    println!("MCP config file path: {}", m.mcp_path().display());
    println!(
        "MCP config exists: {}",
        if m.exists() { "yes" } else { "no" }
    );
    if m.exists() {
        let meta = std::fs::metadata(m.mcp_path())?;
        println!("MCP config file size: {} bytes", meta.len());
    }
    println!("Global config file path: {}", m.app_config_path().display());
    println!(
        "Global config exists: {}",
        if m.app_config_exists() { "yes" } else { "no" }
    );
    if m.app_config_exists() {
        let meta = std::fs::metadata(m.app_config_path())?;
        println!("Global config file size: {} bytes", meta.len());
    }
    Ok(())
}

fn add_examples(path: Option<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let m = mgr(path);
    let added = m.add_examples()?;
    if added == 0 {
        println!("[Info] No new example services were added");
    } else {
        println!("[Success] Added {added} example services");
    }
    Ok(())
}
