use clap::Subcommand;
use mcpstore::{
    client_config::{import_selected_services, inspect_client_config, ClientKind},
    config::ConfigManager,
};

use crate::store_args::{build_store, StoreSourceArgs};

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
        #[arg(long)]
        redis_url: Option<String>,
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

pub async fn run(action: ConfigAction) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::Show { path } => show(path),
        ConfigAction::Validate { path } => validate(path),
        ConfigAction::Init {
            path,
            force,
            with_examples,
            redis_url,
        } => init(path, force, with_examples, redis_url),
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

async fn import_client(
    client: String,
    path: String,
    names_file: String,
    source: StoreSourceArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspection = inspect_client_config(parse_client(&client)?, &path)?;
    let names: Vec<String> = serde_json::from_str(&std::fs::read_to_string(names_file)?)?;
    let services = import_selected_services(&inspection, &names)?;
    let store = build_store(&source)?;
    store.load_from_source().await?;
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
    println!("  Backend: {}", app_config.cache.backend);
    println!("  Namespace: {}", app_config.cache.namespace);
    if let Some(url) = &app_config.cache.url {
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
    redis_url: Option<String>,
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
    m.init(with_examples, redis_url)?;
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
