//! Disk-backed plugin registry. `~/.mcpstore/plugins/<id>/plugin.toml` is the
//! only source of client support: the registry scans it, keeps the enabled and
//! parseable setups as a reloadable snapshot, and reports everything else
//! (disabled, broken) with named reasons for the UI. Install, uninstall,
//! enable and disable call [`reload`]; every reader picks up the new set
//! without a restart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use super::manifest_setup::ManifestSetup;
use super::ClientSetup;
use crate::{Error, FailureCode, Result};

/// One plugin directory as the registry sees it.
#[derive(Debug, Clone, Serialize)]
pub struct PluginReport {
    /// Directory name; for unparseable manifests this is the only identity.
    pub id: String,
    pub label: String,
    pub plugin_version: Option<String>,
    pub spec_version: Option<u32>,
    pub enabled: bool,
    /// Parse or spec failure reason; `None` means usable.
    pub error: Option<String>,
}

/// One machine-detection record for a client id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedClient {
    /// RFC 3339 timestamp.
    pub first_detected: String,
    /// Config file that matched, when detection found one.
    #[serde(default)]
    pub config_path: Option<String>,
    /// `plugin_version` at install/materialize time; display cache only.
    #[serde(default)]
    pub installed_version: Option<String>,
}

/// Persistent detection record and version cache at `plugins/.detections`.
/// A cache, not state: deleting it only costs a re-detect. Enablement lives in
/// `config.toml`, never here.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionTrace {
    /// The bundled plugin set was consumed on first launch; the bundle is
    /// never read again.
    #[serde(default)]
    pub bundle_consumed: bool,
    #[serde(default)]
    pub last_detect: Option<String>,
    #[serde(default)]
    pub clients: BTreeMap<String, DetectedClient>,
    /// Market `plugin_version` per id, cached from the last market fetch.
    #[serde(default)]
    pub market_versions: BTreeMap<String, String>,
    #[serde(default)]
    pub market_fetched_at: Option<String>,
}

static REGISTRY: RwLock<Option<Arc<[Arc<dyn ClientSetup>]>>> = RwLock::new(None);

/// The plugin directory: `~/.mcpstore/plugins/`.
pub fn plugins_dir() -> PathBuf {
    crate::config::resolver::default_user_data_dir().join("plugins")
}

/// Every active client setup (enabled and parseable), in directory order.
/// First access scans lazily so CLI one-shots work without daemon init.
pub fn all() -> Arc<[Arc<dyn ClientSetup>]> {
    if let Some(active) = REGISTRY.read().expect("plugin registry lock").clone() {
        return active;
    }
    let mut guard = REGISTRY.write().expect("plugin registry lock");
    if guard.is_none() {
        let (active, _) = scan_current();
        *guard = Some(active);
    }
    guard.clone().expect("registry just set")
}

/// The active setup whose [`ClientSetup::id`] is `id`.
pub fn by_id(id: &str) -> Option<Arc<dyn ClientSetup>> {
    all().iter().find(|setup| setup.id() == id).cloned()
}

/// Full disk view for the UI: every plugin directory with its status,
/// including the broken and disabled ones `all()` leaves out.
pub fn registry_report() -> Vec<PluginReport> {
    scan_current().1
}

/// Rescan the plugin directory against the enabled list in `config.toml`.
pub fn reload() {
    let (active, _) = scan_current();
    *REGISTRY.write().expect("plugin registry lock") = Some(active);
}

/// Enable a plugin: it must be on disk. Install and enablement are separate
/// acts — installing from the market leaves the plugin disabled.
pub fn enable(id: &str) -> Result<()> {
    if !plugins_dir().join(id).join("plugin.toml").is_file() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("plugin {id} is not installed"),
        ));
    }
    let mut enabled = current_enabled();
    if !enabled.iter().any(|enabled_id| enabled_id == id) {
        enabled.push(id.to_string());
        set_enabled(&enabled)?;
    }
    reload();
    Ok(())
}

/// Refuse to act on a plugin whose client still carries gateway entries.
/// Those files belong to the wire ledger; mutating the plugin behind its
/// back would corrupt them. A corrupt ledger is a hard stop, never a pass.
/// Shared by disable and market uninstall.
pub(crate) fn refuse_while_wired(id: &str, action: &str) -> Result<()> {
    let wired =
        crate::client_wire::load_wired_state(&crate::client_wire::wired_state_default_path())
            .map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("cannot verify wiring for plugin {id}: {error}"),
                )
            })?;
    if wired.entries.iter().any(|entry| entry.client_id == id) {
        return Err(Error::new(
            FailureCode::InvalidInput,
            format!("plugin {id} still has gateway entries; unwire it before {action}"),
        ));
    }
    Ok(())
}

/// Disable a plugin: remove it from the enabled list only, folder untouched.
pub fn disable(id: &str) -> Result<()> {
    refuse_while_wired(id, "disabling")?;
    let mut enabled = current_enabled();
    let before = enabled.len();
    enabled.retain(|enabled_id| enabled_id != id);
    if enabled.len() != before {
        set_enabled(&enabled)?;
    }
    reload();
    Ok(())
}

fn scan_current() -> (Arc<[Arc<dyn ClientSetup>]>, Vec<PluginReport>) {
    let enabled = current_enabled();
    scan(&plugins_dir(), &enabled)
}

pub(crate) fn current_enabled() -> Vec<String> {
    crate::config::ConfigManager::new()
        .load_app_config_or_default()
        .map(|config| config.plugins.enabled)
        .unwrap_or_default()
}

/// Persist the enabled list back to `config.toml`. Callers run this before the
/// daemon serves, so there is no competing writer to race.
pub(crate) fn set_enabled(enabled: &[String]) -> Result<()> {
    let manager = crate::config::ConfigManager::new();
    let mut config = manager.load_app_config_or_default().map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("failed to read config: {error}"),
        )
    })?;
    config.plugins.enabled = enabled.to_vec();
    manager.save_app_config(&config).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("failed to write config: {error}"),
        )
    })
}

/// Desktop first launch: copy detected agents' plugin folders from the bundled
/// directory into the plugin dir, then mark the bundle consumed — it is never
/// read again. A missing bundle means zero detections, never an error.
/// Returns how many plugins were materialized.
pub fn materialize_bundle(bundle_dir: &Path) -> Result<usize> {
    let mut trace = load_trace();
    if trace.bundle_consumed {
        return Ok(0);
    }
    let mut enabled = current_enabled();
    let count = materialize_into(bundle_dir, &plugins_dir(), &mut enabled, &mut trace)?;
    trace.bundle_consumed = true;
    save_trace(&trace)?;
    set_enabled(&enabled)?;
    reload();
    Ok(count)
}

/// The disk half of [`materialize_bundle`], parameterized for tests: detect
/// each bundled manifest on this machine, copy the hits into the plugin dir,
/// enable them, and record them in the trace.
fn materialize_into(
    bundle_dir: &Path,
    plugins: &Path,
    enabled: &mut Vec<String>,
    trace: &mut DetectionTrace,
) -> Result<usize> {
    let mut materialized = 0;
    trace.last_detect = Some(now_rfc3339());
    for path in sorted_subdirs(bundle_dir) {
        let Some(source) = read_manifest(&path) else {
            continue;
        };
        // A bundled manifest that fails to parse is a packaging bug; skipping
        // it must not fail the user's first launch.
        let Ok(setup) = ManifestSetup::from_toml(&source) else {
            continue;
        };
        let Some(detection) = super::detect(&setup) else {
            continue;
        };
        let id = setup.id().to_string();
        let target = plugins.join(&id);
        if !target.exists() {
            copy_dir(&path, &target).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!(
                        "failed to copy plugin {id} into {}: {error}",
                        target.display()
                    ),
                )
            })?;
            materialized += 1;
        }
        trace
            .clients
            .entry(id.clone())
            .or_insert_with(|| DetectedClient {
                first_detected: now_rfc3339(),
                config_path: Some(detection.path.display().to_string()),
                installed_version: Some(setup.plugin_version().to_string()),
            });
        if !enabled.iter().any(|enabled_id| *enabled_id == id) {
            enabled.push(id);
        }
    }
    Ok(materialized)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Classify every plugin folder under `dir`. Directories without a
/// `plugin.toml` are not plugins and are skipped silently; a manifest that
/// fails to parse is reported with its reason instead of failing the scan.
fn scan(dir: &Path, enabled: &[String]) -> (Arc<[Arc<dyn ClientSetup>]>, Vec<PluginReport>) {
    let mut active: Vec<Arc<dyn ClientSetup>> = Vec::new();
    let mut report = Vec::new();
    for path in sorted_subdirs(dir) {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(source) = read_manifest(&path) else {
            continue;
        };
        match ManifestSetup::from_toml(&source) {
            Ok(setup) => {
                let is_enabled = enabled.iter().any(|id| id == setup.id());
                report.push(PluginReport {
                    id: setup.id().to_string(),
                    label: setup.label().to_string(),
                    plugin_version: Some(setup.plugin_version().to_string()),
                    spec_version: Some(setup.spec_version()),
                    enabled: is_enabled,
                    error: None,
                });
                if is_enabled {
                    active.push(Arc::new(setup));
                }
            }
            Err(error) => report.push(PluginReport {
                id: name.to_string(),
                label: name.to_string(),
                plugin_version: None,
                spec_version: None,
                enabled: enabled.iter().any(|id| id == name),
                error: Some(error.to_string()),
            }),
        }
    }
    (active.into(), report)
}

fn sorted_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));
    dirs
}

fn read_manifest(dir: &Path) -> Option<String> {
    let metadata = std::fs::metadata(dir.join("plugin.toml")).ok()?;
    if !metadata.is_file() {
        return None;
    }
    std::fs::read_to_string(dir.join("plugin.toml")).ok()
}

/// Now, RFC 3339 UTC with second precision.
pub(crate) fn now_rfc3339() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map(|seconds| {
            // 2026-01-01T00:00:00Z is 1767225600; days-to-civil conversion.
            let days = seconds / 86400;
            let time = seconds % 86400;
            civil_from_days(days as i64, time)
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn civil_from_days(days: i64, time: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}

pub fn trace_path() -> PathBuf {
    plugins_dir().join(".detections")
}

pub fn load_trace() -> DetectionTrace {
    load_trace_at(&trace_path())
}

pub fn save_trace(trace: &DetectionTrace) -> Result<()> {
    save_trace_at(&trace_path(), trace)
}

fn load_trace_at(path: &Path) -> DetectionTrace {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|source| serde_json::from_str(&source).ok())
        .unwrap_or_default()
}

fn save_trace_at(path: &Path, trace: &DetectionTrace) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("failed to create {}: {error}", parent.display()),
            )
        })?;
    }
    let body = serde_json::to_string_pretty(trace).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("failed to encode trace: {error}"),
        )
    })?;
    std::fs::write(path, body).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("failed to write {}: {error}", path.display()),
        )
    })
}

/// Test-only: replace the registry with setups parsed from the repo's own
/// plugin manifests, so `client_config` and `client_wire` tests never touch a
/// real plugin directory. Idempotent.
#[cfg(test)]
pub(crate) fn install_for_tests() {
    let builtins: &[(&str, &str)] = &[
        (
            "claude",
            include_str!("../../../../../plugins/claude/plugin.toml"),
        ),
        (
            "codex",
            include_str!("../../../../../plugins/codex/plugin.toml"),
        ),
        (
            "opencode",
            include_str!("../../../../../plugins/opencode/plugin.toml"),
        ),
        (
            "cursor",
            include_str!("../../../../../plugins/cursor/plugin.toml"),
        ),
        ("pi", include_str!("../../../../../plugins/pi/plugin.toml")),
        (
            "omp",
            include_str!("../../../../../plugins/omp/plugin.toml"),
        ),
    ];
    let setups: Vec<Arc<dyn ClientSetup>> = builtins
        .iter()
        .map(|(id, source)| {
            Arc::new(
                ManifestSetup::from_toml(source)
                    .unwrap_or_else(|error| panic!("test fixture {id} failed to parse: {error}")),
            ) as Arc<dyn ClientSetup>
        })
        .collect();
    *REGISTRY.write().expect("plugin registry lock") = Some(setups.into());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str) -> String {
        format!(
            r#"
[plugin]
id = "{id}"
label = "Test {id}"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
config_paths = ["~/.{id}.json"]
"#
        )
    }

    fn setup_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mcpstore-registry-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_plugin(dir: &Path, id: &str, source: &str) {
        std::fs::create_dir_all(dir.join(id)).unwrap();
        std::fs::write(dir.join(id).join("plugin.toml"), source).unwrap();
    }

    #[test]
    fn scan_classifies_enabled_disabled_and_errors() {
        let dir = setup_dir("scan");
        write_plugin(&dir, "alpha", &manifest("alpha"));
        write_plugin(&dir, "beta", &manifest("beta"));
        write_plugin(&dir, "broken", "[plugin]\nid = \"broken\"\n"); // unparseable
        std::fs::create_dir_all(dir.join("not-a-plugin")).unwrap(); // no manifest, skipped

        let (active, report) = scan(&dir, &["alpha".to_string()]);
        assert_eq!(active.iter().map(|s| s.id()).collect::<Vec<_>>(), ["alpha"]);
        assert_eq!(report.len(), 3);
        let broken = report.iter().find(|r| r.id == "broken").unwrap();
        assert!(broken.error.is_some());
        assert!(!broken.enabled);
        assert!(!report.iter().any(|r| r.id == "not-a-plugin"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scan_missing_directory_is_empty_not_error() {
        let (active, report) = scan(Path::new("/nonexistent-mcpstore-dir"), &[]);
        assert!(active.is_empty());
        assert!(report.is_empty());
    }

    #[test]
    fn trace_roundtrips_through_disk() {
        let path = std::env::temp_dir().join(format!("mcpstore-trace-{}.json", std::process::id()));
        let mut trace = DetectionTrace::default();
        trace.bundle_consumed = true;
        trace.clients.insert(
            "claude".into(),
            DetectedClient {
                first_detected: "2026-10-27T00:00:00Z".into(),
                config_path: Some("/home/x/.claude.json".into()),
                installed_version: Some("1.0.0".into()),
            },
        );
        save_trace_at(&path, &trace).unwrap();
        assert_eq!(load_trace_at(&path), trace);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn now_rfc3339_has_second_precision() {
        let now = now_rfc3339();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z'), "{now}");
    }

    #[test]
    fn materialize_copies_detected_plugins_and_enables_them() {
        let root = std::env::temp_dir().join(format!("mcpstore-mat-{}", std::process::id()));
        let bundle = root.join("bundle");
        let plugins = root.join("plugins");
        std::fs::create_dir_all(bundle.join("fake-agent")).unwrap();
        // An absolute config path (no `~`) so detect hits a file we control.
        let config = root.join("fake-agent.json");
        std::fs::write(&config, "{}").unwrap();
        let source = format!(
            r#"
[plugin]
id = "fake-agent"
label = "Fake Agent"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
config_paths = ["{}"]
"#,
            config.display()
        );
        std::fs::write(bundle.join("fake-agent").join("plugin.toml"), source).unwrap();
        // A bundled plugin nothing on this machine matches must not copy.
        std::fs::create_dir_all(bundle.join("absent-agent")).unwrap();
        std::fs::write(
            bundle.join("absent-agent").join("plugin.toml"),
            manifest("absent-agent"),
        )
        .unwrap();

        let mut enabled = Vec::new();
        let mut trace = DetectionTrace::default();
        let count = materialize_into(&bundle, &plugins, &mut enabled, &mut trace).unwrap();
        assert_eq!(count, 1);
        assert!(plugins.join("fake-agent").join("plugin.toml").is_file());
        assert!(!plugins.join("absent-agent").exists());
        assert_eq!(enabled, ["fake-agent"]);
        assert_eq!(
            trace.clients["fake-agent"].installed_version.as_deref(),
            Some("1.0.0")
        );

        // Existing install wins over a re-copy; detection is recorded once.
        let count = materialize_into(&bundle, &plugins, &mut enabled, &mut trace).unwrap();
        assert_eq!(count, 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn materialize_into_with_missing_bundle_is_zero() {
        let mut enabled = Vec::new();
        let mut trace = DetectionTrace::default();
        let count = materialize_into(
            Path::new("/nonexistent-mcpstore-bundle"),
            Path::new("/nonexistent-mcpstore-plugins"),
            &mut enabled,
            &mut trace,
        )
        .unwrap();
        assert_eq!(count, 0);
        assert!(enabled.is_empty());
    }
}
