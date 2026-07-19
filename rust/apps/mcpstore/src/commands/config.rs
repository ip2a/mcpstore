use clap::Subcommand;
use mcpstore::{
    client_config::{
        apply_config_change, inspect_client_config, plan_add_entries, ClientConfigInspection,
        ClientEntryPlan, ClientEntrySpec, ClientEntryStatus, ClientKind, ConfigChangeReceipt,
    },
    config::ConfigManager,
};

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
    InspectClient {
        #[arg(long)]
        client: String,
        #[arg(long)]
        path: String,
    },
    PlanClient {
        #[arg(long)]
        client: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        entries_file: String,
    },
    ApplyClient {
        #[arg(long)]
        client: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        entries_file: String,
        #[arg(long)]
        receipt_file: String,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
    UndoClient {
        #[arg(long)]
        receipt_file: String,
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
        ConfigAction::InspectClient { client, path } => inspect_client(client, path),
        ConfigAction::PlanClient {
            client,
            path,
            entries_file,
        } => plan_client(client, path, entries_file),
        ConfigAction::ApplyClient {
            client,
            path,
            entries_file,
            receipt_file,
            yes,
        } => apply_client(client, path, entries_file, receipt_file, yes),
        ConfigAction::UndoClient { receipt_file } => undo_client(receipt_file),
    }
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
fn read_entries(path: &str) -> Result<Vec<ClientEntrySpec>, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}
fn safe_plan_json(
    inspection: &ClientConfigInspection,
    plans: &[ClientEntryPlan],
) -> serde_json::Value {
    serde_json::json!({"client": format!("{:?}", inspection.client), "path": inspection.path, "content_hash": inspection.content_hash, "plans": plans.iter().map(|plan| serde_json::json!({"name": plan.name, "kind": format!("{:?}", plan.kind), "status": format!("{:?}", plan.status), "fields": plan.proposed.as_object().map(|v| v.keys().collect::<Vec<_>>()).unwrap_or_default(), "unsupported_fields": plan.unsupported_fields})).collect::<Vec<_>>()})
}
fn inspect_client(client: String, path: String) -> Result<(), Box<dyn std::error::Error>> {
    let inspection = inspect_client_config(parse_client(&client)?, &path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"client": client, "path": inspection.path, "format": format!("{:?}", inspection.format), "content_hash": inspection.content_hash, "services": inspection.services.iter().map(|s| serde_json::json!({"name": s.name, "fields": s.config.as_object().map(|v| v.keys().collect::<Vec<_>>()).unwrap_or_default()})).collect::<Vec<_>>(), "unsupported_fields": inspection.unsupported_fields})
        )?
    );
    Ok(())
}
fn plan_client(
    client: String,
    path: String,
    entries_file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspection = inspect_client_config(parse_client(&client)?, &path)?;
    let plans = plan_add_entries(&inspection, read_entries(&entries_file)?);
    println!(
        "{}",
        serde_json::to_string_pretty(&safe_plan_json(&inspection, &plans))?
    );
    Ok(())
}
fn apply_client(
    client: String,
    path: String,
    entries_file: String,
    receipt_file: String,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspection = inspect_client_config(parse_client(&client)?, &path)?;
    let plans = plan_add_entries(&inspection, read_entries(&entries_file)?);
    println!(
        "{}",
        serde_json::to_string_pretty(&safe_plan_json(&inspection, &plans))?
    );
    if plans.iter().any(|plan| {
        matches!(
            plan.status,
            ClientEntryStatus::Conflict | ClientEntryStatus::Unsupported
        )
    }) {
        return Err("plan contains conflict or unsupported entries; nothing was written".into());
    }
    if !yes {
        eprintln!("Apply this plan? [y/N]");
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            return Ok(());
        }
    }
    if let Some(receipt) = apply_config_change(&inspection, &plans)? {
        std::fs::write(receipt_file, serde_json::to_vec_pretty(&receipt)?)?;
        println!("configuration applied");
    } else {
        println!("configuration already matches plan");
    }
    Ok(())
}
fn undo_client(receipt_file: String) -> Result<(), Box<dyn std::error::Error>> {
    let receipt: ConfigChangeReceipt = serde_json::from_slice(&std::fs::read(&receipt_file)?)?;
    mcpstore::client_config::undo_last_change(&receipt)?;
    println!("configuration undone");
    Ok(())
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
    if let Some(redis_url) = &app_config.cache.redis_url {
        println!("  Redis URL: {}", redis_url);
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
