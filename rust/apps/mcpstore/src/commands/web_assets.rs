use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_web_assets.rs"));
}

const ENV_WEB_ASSETS_DIR: &str = "MCPSTORE_WEB_ASSETS_DIR";

pub fn has_assets() -> bool {
    !embedded::ASSETS.is_empty() || override_assets_dir().is_some()
}

pub fn has_override_assets() -> bool {
    override_assets_dir().is_some()
}

pub fn find(path: &str) -> Option<(&'static [u8], &'static str)> {
    embedded::ASSETS
        .iter()
        .find(|asset| asset.path == path)
        .map(|asset| (asset.contents, asset.mime))
}

pub fn response_for_asset(path: &str) -> Response {
    if let Some(dir) = override_assets_dir() {
        return response_from_dir(&dir, path);
    }
    match find(path) {
        Some((contents, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(mime))
            .body(Body::from(contents.to_vec()))
            .expect("static asset response is valid"),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn override_assets_dir() -> Option<PathBuf> {
    std::env::var_os(ENV_WEB_ASSETS_DIR)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_dir())
}

fn response_from_dir(dir: &Path, path: &str) -> Response {
    let Some(safe_path) = safe_asset_path(dir, path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(&safe_path) {
        Ok(contents) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime_for_path(path)),
            )
            .body(Body::from(contents))
            .expect("static asset response is valid"),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn safe_asset_path(base: &Path, path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    if relative.is_absolute() {
        return None;
    }

    let mut safe = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(base.join(safe))
}

fn mime_for_path(path: &str) -> &'static str {
    if path.ends_with(".html") || path.is_empty() {
        return "text/html; charset=utf-8";
    }
    if path.ends_with(".js") {
        return "text/javascript; charset=utf-8";
    }
    if path.ends_with(".css") {
        return "text/css; charset=utf-8";
    }
    if path.ends_with(".svg") {
        return "image/svg+xml";
    }
    if path.ends_with(".json") {
        return "application/json; charset=utf-8";
    }
    if path.ends_with(".ico") {
        return "image/x-icon";
    }
    if path.ends_with(".png") {
        return "image/png";
    }
    if path.ends_with(".woff2") {
        return "font/woff2";
    }
    if path.ends_with(".wasm") {
        return "application/wasm";
    }
    "application/octet-stream"
}

/// SPA 静态回退：命中嵌入资源返回资源；带扩展名未命中返回 404；其余回 index.html。
pub async fn serve_react_app(request: axum::extract::Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    let asset_path = if path.is_empty() { "index.html" } else { path };

    if has_override_assets() && asset_path == "index.html" {
        return response_for_asset(asset_path);
    }

    if find(asset_path).is_some() {
        return response_for_asset(asset_path);
    }

    if !has_assets() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "mcpstore web assets are missing. Run `cd web && npm run build` before building or set MCPSTORE_WEB_ASSETS_DIR.",
        )
            .into_response();
    }

    if Path::new(asset_path).extension().is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }

    response_for_asset("index.html")
}
