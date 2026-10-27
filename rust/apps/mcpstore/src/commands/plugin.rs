use clap::Subcommand;

/// Plugin management over the daemon socket: list, market, detect,
/// install/uninstall, enable/disable. Every operation hot-applies in the
/// running daemon; the CLI never touches plugin files directly.
#[derive(Subcommand)]
pub enum PluginAction {
    /// Installed plugins with status and the cached market versions.
    List,
    /// The market index, refreshed: versions, install/update status.
    Market,
    /// Detect uninstalled clients on this machine (market discovery rules).
    Detect,
    /// Install a plugin from the market (lands disabled).
    Install { id: String },
    /// Remove a plugin folder and its traces (refused while wired).
    Uninstall { id: String },
    /// Enable an installed plugin.
    Enable { id: String },
    /// Disable a plugin (refused while wired).
    Disable { id: String },
}

pub async fn run(action: PluginAction) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if !crate::daemon::protocol::is_daemon_running() {
        return Err("daemon 未运行；插件操作需经 daemon。先运行 mcpstore start".into());
    }
    let mut client = crate::daemon::client::connect_admin(None).await?;
    let result = match action {
        PluginAction::List => client.plugin_list().await?,
        PluginAction::Market => client.plugin_market().await?,
        PluginAction::Detect => client.plugin_detect().await?,
        PluginAction::Install { id } => client.plugin_install(&id).await?,
        PluginAction::Uninstall { id } => client.plugin_uninstall(&id).await?,
        PluginAction::Enable { id } => client.plugin_enable(&id).await?,
        PluginAction::Disable { id } => client.plugin_disable(&id).await?,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
