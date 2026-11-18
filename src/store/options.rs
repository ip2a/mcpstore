use super::store_config::JsonStoreConfig;
use crate::config::DiagnosticsConfig;

#[derive(Clone, Debug)]
pub struct CoreDiagnosticsOptions {
    pub requests: CoreRequestCaptureOptions,
    pub upstream_frames: CoreFrameCaptureOptions,
    pub event_history: CoreEventHistoryOptions,
    pub stdio_stderr: CoreStdioStderrOptions,
}

#[derive(Clone, Debug)]
pub struct CoreRequestCaptureOptions {
    pub capture_tool: bool,
    pub capture_payload: bool,
}

#[derive(Clone, Debug)]
pub struct CoreFrameCaptureOptions {
    pub enabled: bool,
    pub capture_payload: bool,
}

#[derive(Clone, Debug)]
pub struct CoreEventHistoryOptions {
    pub enabled: bool,
    pub capacity: usize,
}

#[derive(Clone, Debug)]
pub struct CoreStdioStderrOptions {
    pub enabled: bool,
    pub tail_lines: usize,
    pub forward_to_runtime_log: bool,
    pub include_in_error: bool,
}

impl Default for CoreDiagnosticsOptions {
    fn default() -> Self {
        Self::from(&DiagnosticsConfig::default())
    }
}

impl From<&DiagnosticsConfig> for CoreDiagnosticsOptions {
    fn from(config: &DiagnosticsConfig) -> Self {
        Self {
            requests: CoreRequestCaptureOptions {
                capture_tool: config.requests.enabled && config.requests.capture_tool,
                capture_payload: config.requests.capture_payload,
            },
            upstream_frames: CoreFrameCaptureOptions {
                enabled: config.frames.enabled && config.frames.capture_upstream,
                capture_payload: config.frames.capture_payload,
            },
            event_history: CoreEventHistoryOptions {
                enabled: config.event_history.enabled,
                capacity: config.event_history.capacity,
            },
            stdio_stderr: CoreStdioStderrOptions {
                enabled: config.stdio_stderr.enabled,
                tail_lines: config.stdio_stderr.tail_lines,
                forward_to_runtime_log: config.stdio_stderr.forward_to_runtime_log,
                include_in_error: config.stdio_stderr.include_in_error,
            },
        }
    }
}

/// Panel role: a setup parameter that decides this process's behavior. kv is always the source of truth; the store
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
            .map(|name| {
                name.to_string_lossy()
                    .trim()
                    .trim_end_matches('.')
                    .to_string()
            })
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
    pub diagnostics: Option<CoreDiagnosticsOptions>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            store: None,
            namespace: None,
            panel: PanelRole::ControlPanel,
            diagnostics: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_diagnostics_mapping_combines_global_and_source_switches() {
        let defaults = CoreDiagnosticsOptions::default();
        assert!(defaults.requests.capture_tool);
        assert!(defaults.requests.capture_payload);
        assert!(defaults.upstream_frames.enabled);
        assert!(defaults.upstream_frames.capture_payload);
        assert!(defaults.event_history.enabled);
        assert_eq!(defaults.event_history.capacity, 10_000);
        assert!(defaults.stdio_stderr.enabled);
        assert_eq!(defaults.stdio_stderr.tail_lines, 50);
        assert!(defaults.stdio_stderr.forward_to_runtime_log);
        assert!(defaults.stdio_stderr.include_in_error);

        let mut config = DiagnosticsConfig::default();
        config.requests.enabled = false;
        config.frames.capture_upstream = false;
        config.frames.capture_payload = false;
        config.event_history.capacity = 17;
        config.stdio_stderr.tail_lines = 0;
        let options = CoreDiagnosticsOptions::from(&config);
        assert!(!options.requests.capture_tool);
        assert!(!options.upstream_frames.enabled);
        assert!(!options.upstream_frames.capture_payload);
        assert_eq!(options.event_history.capacity, 17);
        assert_eq!(options.stdio_stderr.tail_lines, 0);
    }

    #[test]
    fn default_panel_id_is_a_non_empty_name() {
        let id = PanelRole::default_panel_id();
        assert!(!id.is_empty());
        assert!(!id.contains(char::is_whitespace));
    }
}
