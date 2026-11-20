//! Directory listing for the panel-local path browser (plugin upload and
//! similar pickers). Lists immediate subdirectories of an absolute path;
//! defaults to the user's home directory when no path is given.

use std::path::PathBuf;
use std::sync::Arc;

use super::envelope::{success, ApiError, ApiResult};
use super::PanelCtx;
use axum::extract::{Query, State};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(super) struct DirectoryQuery {
    path: Option<String>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn directory_listing(requested_path: Option<&str>) -> Result<Value, ApiError> {
    let requested = requested_path
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let candidate = match requested {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(ApiError::invalid_request(
                    "Directory path must be absolute.",
                ));
            }
            path
        }
        None => home_dir().unwrap_or_else(|| PathBuf::from(".")),
    };

    let path = candidate.canonicalize().map_err(|error| {
        ApiError::invalid_request(format!(
            "Cannot access directory {}: {error}",
            candidate.display()
        ))
    })?;
    if !path.is_dir() {
        return Err(ApiError::invalid_request(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    let mut directories = std::fs::read_dir(&path)
        .map_err(|error| ApiError::internal(format!("Cannot read directory: {error}")))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let entry_path = entry.path();
            entry_path.is_dir().then(|| {
                json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry_path.to_string_lossy(),
                })
            })
        })
        .collect::<Vec<_>>();
    directories.sort_by(|left, right| {
        let left_name = left["name"].as_str().unwrap_or_default();
        let right_name = right["name"].as_str().unwrap_or_default();
        left_name
            .to_lowercase()
            .cmp(&right_name.to_lowercase())
            .then_with(|| left_name.cmp(right_name))
    });

    Ok(json!({
        "path": path.to_string_lossy(),
        "parent": path.parent().map(|parent| parent.to_string_lossy()),
        "directories": directories,
    }))
}

pub(super) async fn directories_list<T: PanelCtx>(
    State(_state): State<Arc<T>>,
    Query(query): Query<DirectoryQuery>,
) -> ApiResult {
    let listing = directory_listing(query.path.as_deref())?;
    Ok(success("Directory listing fetched successfully", listing))
}
