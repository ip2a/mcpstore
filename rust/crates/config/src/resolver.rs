use std::path::{Path, PathBuf};

pub const MCP_CONFIG_FILENAME: &str = "mcp.json";
pub const APP_CONFIG_FILENAME: &str = "config.toml";
const APP_NAME: &str = "mcpstore";

pub fn resolve_config_path() -> PathBuf {
    resolve_mcp_config_path()
}

pub fn resolve_mcp_config_path() -> PathBuf {
    let candidates = search_paths(MCP_CONFIG_FILENAME);
    for path in &candidates {
        if path.exists() {
            return path.clone();
        }
    }
    default_user_data_dir().join(MCP_CONFIG_FILENAME)
}

pub fn app_config_path_for_mcp_path(mcp_path: impl AsRef<Path>) -> PathBuf {
    mcp_path
        .as_ref()
        .parent()
        .map(|parent| parent.join(APP_CONFIG_FILENAME))
        .unwrap_or_else(|| PathBuf::from(APP_CONFIG_FILENAME))
}

fn search_paths(filename: &str) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(3);

    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(filename));
    }

    paths.push(default_user_data_dir().join(filename));

    paths.push(system_config_dir().join(filename));
    paths
}

pub fn default_user_data_dir() -> PathBuf {
    home_dir()
        .map(|home| home.join(format!(".{APP_NAME}")))
        .unwrap_or_else(|| PathBuf::from(format!(".{APP_NAME}")))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn system_config_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support").join(APP_NAME)
    } else if cfg!(target_os = "windows") {
        PathBuf::from("C:\\ProgramData").join(APP_NAME)
    } else {
        PathBuf::from("/etc").join(APP_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_a_path() {
        let p = resolve_config_path();
        assert!(p.to_str().unwrap().contains("mcp.json"));
    }

    #[test]
    fn app_config_path_uses_mcp_parent() {
        let p = app_config_path_for_mcp_path("/tmp/custom/mcp.json");
        assert_eq!(p, PathBuf::from("/tmp/custom/config.toml"));
    }
}
