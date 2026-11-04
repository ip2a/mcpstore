//! Market client: everything plugin installation needs from the outside —
//! the index (`index.json` in the plugin repo), one-plugin downloads
//! (`dist/<id>.zip`, blake3-checked), market-driven discovery, and the
//! market version cache in the trace file. Base URLs come from
//! `config.toml [plugins].market_urls` and are tried in order.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::manifest_setup::ManifestSetup;
use super::registry::{
    current_enabled, load_trace, now_rfc3339, plugins_dir, reload, save_trace, set_enabled,
    DetectedClient,
};
use super::{detect_by_discovery, ClientSetup, DetectedBy};
use crate::{Error, FailureCode, Result};

/// One entry of the market index.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketEntry {
    pub id: String,
    pub label: String,
    pub plugin_version: String,
    pub spec_version: u32,
    #[serde(default)]
    pub description: Option<String>,
    /// Repo-relative zip path, e.g. `dist/claude.zip`.
    pub download: String,
    /// blake3 of the zip, hex encoded.
    pub digest: String,
    #[serde(default)]
    pub discover: Option<super::manifest_setup::DiscoverSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketIndex {
    pub spec_version: u32,
    #[serde(default)]
    pub generated_at: Option<String>,
    pub plugins: Vec<MarketEntry>,
}

/// One market entry as the UI sees it: its version, whether it is installed
/// here, and whether the market is ahead of the installed copy.
#[derive(Debug, Serialize)]
pub struct MarketListing {
    pub id: String,
    pub label: String,
    pub plugin_version: String,
    pub description: Option<String>,
    pub installed: Option<String>,
    pub update_available: bool,
}

/// A client found on this machine whose plugin is not installed.
#[derive(Debug, Serialize)]
pub struct DetectHit {
    pub id: String,
    pub label: String,
    /// Config file or directory that matched.
    pub path: String,
    pub by: DetectedBy,
}

/// Fetch and parse `index.json`, returning it with the base URL that served
/// it; every URL in the list is named when all of them fail.
async fn fetch_index(urls: &[String]) -> Result<(MarketIndex, String)> {
    let client = reqwest::Client::new();
    let mut failures = Vec::new();
    for base in urls {
        let base = normalize_base(base);
        let index_url = format!("{base}index.json");
        let outcome = async {
            let response = client.get(&index_url).send().await?;
            let response = response.error_for_status()?;
            response.json::<MarketIndex>().await
        }
        .await;
        match outcome {
            Ok(index) => return Ok((index, base)),
            Err(error) => failures.push(format!("{index_url}: {error}")),
        }
    }
    Err(Error::new(
        FailureCode::Internal,
        format!("market unreachable: {}", failures.join("; ")),
    ))
}

fn normalize_base(url: &str) -> String {
    let trimmed = url.trim_end();
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
}

/// Segment-wise numeric semver compare; callers validate the triplet shape.
pub(crate) fn version_gt(candidate: &str, installed: &str) -> bool {
    let parse = |version: &str| -> [u64; 3] {
        let mut segments = [0u64; 3];
        for (index, part) in version.split('.').take(3).enumerate() {
            segments[index] = part.parse().unwrap_or(0);
        }
        segments
    };
    let (candidate, installed) = (parse(candidate), parse(installed));
    candidate > installed
}

fn installed_version(id: &str) -> Option<String> {
    let source = std::fs::read_to_string(plugins_dir().join(id).join("plugin.toml")).ok()?;
    ManifestSetup::from_toml(&source)
        .ok()
        .map(|setup| setup.plugin_version().to_string())
}

/// Pull the market index, refresh the version cache in the trace, and list
/// every market entry with its install/update status. Updates happen when the
/// user asks for one — this only reports.
pub async fn list_market(urls: &[String]) -> Result<Vec<MarketListing>> {
    let (index, _) = fetch_index(urls).await?;
    let mut trace = load_trace();
    let mut listings = Vec::new();
    for entry in &index.plugins {
        trace
            .market_versions
            .insert(entry.id.clone(), entry.plugin_version.clone());
        let installed = installed_version(&entry.id);
        let update_available = installed
            .as_deref()
            .map(|installed| version_gt(&entry.plugin_version, installed))
            .unwrap_or(false);
        listings.push(MarketListing {
            id: entry.id.clone(),
            label: entry.label.clone(),
            plugin_version: entry.plugin_version.clone(),
            description: entry.description.clone(),
            installed,
            update_available,
        });
    }
    trace.market_fetched_at = Some(now_rfc3339());
    save_trace(&trace)?;
    Ok(listings)
}

/// Run the market's discovery rules against this machine and report every
/// matched client whose plugin is not installed. Never installs anything.
pub async fn detect_from_market(urls: &[String]) -> Result<Vec<DetectHit>> {
    let (index, _) = fetch_index(urls).await?;
    let installed: HashSet<String> = super::registry::registry_report()
        .into_iter()
        .map(|report| report.id)
        .collect();
    let mut trace = load_trace();
    trace.last_detect = Some(now_rfc3339());
    let mut hits = Vec::new();
    for entry in &index.plugins {
        if installed.contains(&entry.id) {
            continue;
        }
        let Some(discover) = &entry.discover else {
            continue;
        };
        let Some(detection) = detect_by_discovery(
            discover.platform_config_paths(),
            discover.platform_config_dirs(),
            discover.cli_commands.clone(),
        ) else {
            continue;
        };
        trace
            .clients
            .entry(entry.id.clone())
            .or_insert_with(|| DetectedClient {
                first_detected: now_rfc3339(),
                config_path: Some(detection.path.display().to_string()),
                installed_version: None,
            });
        hits.push(DetectHit {
            id: entry.id.clone(),
            label: entry.label.clone(),
            path: detection.path.display().to_string(),
            by: detection.by,
        });
    }
    save_trace(&trace)?;
    Ok(hits)
}

/// Install one plugin from the market: download, verify, stage, validate the
/// manifest (id and spec), then swap into place. The plugin lands disabled —
/// enablement is a separate act.
pub async fn install(id: &str, urls: &[String]) -> Result<()> {
    let (index, base) = fetch_index(urls).await?;
    let entry = index
        .plugins
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| Error::new(FailureCode::Internal, format!("market has no plugin {id}")))?;
    let target = plugins_dir().join(id);
    if target.exists() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("plugin {id} is already installed; uninstall it first"),
        ));
    }

    let client = reqwest::Client::new();
    let zip_url = format!("{base}{}", entry.download);
    let bytes = client
        .get(&zip_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| Error::new(FailureCode::Internal, format!("{zip_url}: {error}")))?
        .bytes()
        .await
        .map_err(|error| Error::new(FailureCode::Internal, format!("{zip_url}: {error}")))?;
    let digest = blake3::hash(&bytes).to_hex();
    if digest.as_str() != entry.digest {
        return Err(Error::new(
            FailureCode::Internal,
            format!(
                "plugin {id}: download digest {digest} does not match index {}",
                entry.digest
            ),
        ));
    }

    // Stage, then swap: a failed install never leaves a half plugin in place.
    let staging = staging_path(id);
    let _ = std::fs::remove_dir_all(&staging);
    unpack_zip(&bytes, &staging)?;
    let manifest_source =
        std::fs::read_to_string(staging.join("plugin.toml")).map_err(|error| {
            let _ = std::fs::remove_dir_all(&staging);
            Error::new(
                FailureCode::Internal,
                format!("plugin {id}: archive has no plugin.toml at its root: {error}"),
            )
        })?;
    let setup = match ManifestSetup::from_toml(&manifest_source) {
        Ok(setup) if setup.id() == id => setup,
        Ok(setup) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(Error::new(
                FailureCode::Internal,
                format!("plugin {id}: archive manifest declares id {}", setup.id()),
            ));
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(Error::new(
                FailureCode::Internal,
                format!("plugin {id}: {error}"),
            ));
        }
    };
    drop(setup);

    std::fs::rename(&staging, &target).map_err(|error| {
        let _ = std::fs::remove_dir_all(&staging);
        Error::new(
            FailureCode::Internal,
            format!(
                "plugin {id}: failed to move into {}: {error}",
                target.display()
            ),
        )
    })?;

    let mut trace = load_trace();
    if let Some(record) = trace.clients.get_mut(id) {
        record.installed_version = Some(entry.plugin_version.clone());
    }
    save_trace(&trace)?;
    reload();
    Ok(())
}

/// Uninstall one plugin: remove the folder and every trace of it. Refuses
/// while the client still has gateway entries — those files are owned by the
/// wire ledger.
pub async fn uninstall(id: &str) -> Result<()> {
    super::registry::refuse_while_wired(id, "uninstalling")?;
    let target = plugins_dir().join(id);
    if !target.is_dir() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("plugin {id} is not installed"),
        ));
    }
    std::fs::remove_dir_all(&target).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!(
                "plugin {id}: failed to remove {}: {error}",
                target.display()
            ),
        )
    })?;

    let mut trace = load_trace();
    trace.clients.remove(id);
    trace.market_versions.remove(id);
    save_trace(&trace)?;

    let mut enabled = current_enabled();
    enabled.retain(|enabled_id| enabled_id != id);
    set_enabled(&enabled)?;
    reload();
    Ok(())
}

fn staging_path(id: &str) -> PathBuf {
    plugins_dir().join(format!(".staging-{id}-{}", std::process::id()))
}

fn unpack_zip(bytes: &[u8], target: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("invalid zip archive: {error}"),
        )
    })?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            Error::new(FailureCode::Internal, format!("invalid zip entry: {error}"))
        })?;
        // enclosed_name rejects paths escaping the archive root (zip slip).
        let Some(relative) = file.enclosed_name() else {
            continue;
        };
        let out = target.join(relative);
        if file.is_dir() {
            std::fs::create_dir_all(&out).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("failed to create {}: {error}", out.display()),
                )
            })?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    Error::new(
                        FailureCode::Internal,
                        format!("failed to create {}: {error}", parent.display()),
                    )
                })?;
            }
            let mut out_file = std::fs::File::create(&out).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("failed to create {}: {error}", out.display()),
                )
            })?;
            std::io::copy(&mut file, &mut out_file).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("failed to extract {}: {error}", out.display()),
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_gt_compares_segment_wise() {
        assert!(version_gt("1.2.10", "1.2.9"));
        assert!(version_gt("2.0.0", "1.9.9"));
        assert!(!version_gt("1.0.0", "1.0.0"));
        assert!(!version_gt("0.9.9", "1.0.0"));
    }

    #[test]
    fn normalize_base_keeps_trailing_slash() {
        assert_eq!(normalize_base("https://x/y/"), "https://x/y/");
        assert_eq!(normalize_base("https://x/y"), "https://x/y/");
    }
}
