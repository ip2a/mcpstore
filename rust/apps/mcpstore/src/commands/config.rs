use clap::Subcommand;
use mcpstore::config::ConfigManager;

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
