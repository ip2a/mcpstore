use super::store_config::JsonStoreConfig;

/// Panel role: a setup parameter that decides this process's behavior. kv is always the source of truth; backend
/// is just a deployment parameter; the role decides who consumes events and who executes.
///
/// - Control panel: mounts the self-healing supervisor at setup, subscribes to the service_events / tool_call_requests
///   ChangeFeeds and consumes/executes; with config_path, reads the seed at startup and flushes after consuming.
/// - Data panel: no subscription, no execution; queries read kv directly, tool calls are delegated
///   to the control panel through the shared store.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelRole {
    ControlPanel,
    DataPanel { panel_id: String },
}

impl Default for PanelRole {
    fn default() -> Self {
        Self::ControlPanel
    }
}

impl PanelRole {
    /// Default data-panel identity: the machine hostname. Stable across restarts on real
    /// machines — containers often get random hostnames, so a random-looking default means
    /// the operator should set an explicit panel_id.
    pub fn default_panel_id() -> String {
        hostname::get()
            .map(|name| name.to_string_lossy().trim().trim_end_matches('.').to_string())
            .ok()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "data-panel".to_string())
    }
}

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub store: Option<JsonStoreConfig>,
    pub namespace: Option<String>,
    /// Panel role (default ControlPanel: single-machine and cloud both default to control panel;
    /// a data panel must declare panel_id explicitly).
    pub panel: PanelRole,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            store: None,
            namespace: None,
            panel: PanelRole::ControlPanel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_panel_id_is_a_non_empty_name() {
        let id = PanelRole::default_panel_id();
        assert!(!id.is_empty());
        assert!(!id.contains(char::is_whitespace));
    }
}
