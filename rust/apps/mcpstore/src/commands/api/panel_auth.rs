//! Panel token auth. Auto-enabled when the panel binds a non-loopback host:
//! every `/api` request then requires `Authorization: Bearer <token>` with the
//! token from `<data dir>/panel/auth-token` (generated on first use, 0600).
//! The static frontend stays public so the SPA can load and show the token
//! prompt. On a loopback bind no token is required; instead foreign Host
//! headers are rejected (DNS rebinding guard).

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderValue},
    middleware::Next,
    response::{IntoResponse, Response},
};
use mcpstore::config::ConfigManager;
use mcpstore::error::{Error, FailureCode};
use rand::RngCore;
use subtle::ConstantTimeEq;

use super::envelope::ApiError;

/// What the guard enforces, fixed at startup from the bind address.
#[derive(Clone, Default)]
pub(crate) struct PanelAuthGuard {
    /// Some(token) => /api requires a matching Bearer credential.
    bearer: Option<Arc<String>>,
    /// Loopback bind: reject non-loopback Host headers.
    loopback_host_only: bool,
}

impl PanelAuthGuard {
    /// Build the guard for a panel whose bind address resolved as
    /// `is_loopback`. Non-loopback binds turn on token auth (creating the
    /// token file if absent); loopback binds turn on the Host guard.
    pub(crate) fn for_bind(
        is_loopback: bool,
        manager: &ConfigManager,
    ) -> Result<Self, Error> {
        if is_loopback {
            Ok(Self {
                bearer: None,
                loopback_host_only: true,
            })
        } else {
            Ok(Self {
                bearer: Some(Arc::new(load_or_create_token(manager)?)),
                loopback_host_only: false,
            })
        }
    }

    pub(crate) fn token_path(manager: &ConfigManager) -> PathBuf {
        manager
            .app_config_path()
            .parent()
            .map(|dir| dir.join("panel").join("auth-token"))
            .unwrap_or_else(|| PathBuf::from("panel/auth-token"))
    }
}

/// Token file: single line, 64 hex chars, 0600.
pub(crate) fn load_or_create_token(manager: &ConfigManager) -> Result<String, Error> {
    let path = PanelAuthGuard::token_path(manager);
    if let Ok(token) = std::fs::read_to_string(&path) {
        let token = token.trim();
        if token.is_empty() {
            return Err(Error::new(
                FailureCode::ConfigInvalid,
                format!("panel auth token file {} is empty", path.display()),
            ));
        }
        return Ok(token.to_string());
    }

    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                FailureCode::ConfigInvalid,
                format!("cannot create panel auth directory: {error}"),
            )
        })?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut file| writeln!(file, "{token}"))
            .map_err(|error| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    format!("cannot write panel auth token file: {error}"),
                )
            })?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, format!("{token}\n")).map_err(|error| {
        Error::new(
            FailureCode::ConfigInvalid,
            format!("cannot write panel auth token file: {error}"),
        )
    })?;
    println!("generated panel auth token: {}", path.display());
    Ok(token)
}

pub(crate) async fn enforce(
    State(auth): State<PanelAuthGuard>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if auth.loopback_host_only {
        let host_ok = request
            .headers()
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .map(is_loopback_host)
            // HTTP/1.0 without a Host header: nothing to rebind.
            .unwrap_or(true);
        if !host_ok {
            return ApiError::from_store(Error::new(
                FailureCode::ConnectionScope,
                "panel is bound to loopback; requests with a foreign Host header are rejected",
            ))
            .into_response();
        }
    }

    if let Some(token) = &auth.bearer {
        let supplied = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        let matches = supplied.is_some_and(|supplied| {
            supplied.as_bytes().ct_eq(token.as_bytes()).into()
        });
        if !matches {
            let mut response = ApiError::from_store(Error::new(
                FailureCode::PanelAuthRequired,
                "panel authentication required; supply the token from panel/auth-token as a Bearer credential",
            ))
            .into_response();
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer"),
            );
            return response;
        }
    }

    next.run(request).await
}

/// Host header value ("host", "host:port", "[::1]:port") → is this machine.
pub(crate) fn is_loopback_host(host_header: &str) -> bool {
    let trimmed = host_header.trim();
    let host = if trimmed.starts_with('[') {
        trimmed
            .split(']')
            .next()
            .unwrap_or(trimmed)
            .trim_start_matches('[')
    } else {
        trimmed.split(':').next().unwrap_or(trimmed)
    };
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "::1"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_values_are_recognized() {
        for host in ["127.0.0.1", "127.0.0.1:2828", "localhost", "localhost:2828", "[::1]:2828", "[::1]", "LOCALHOST:80"] {
            assert!(is_loopback_host(host), "{host}");
        }
        for host in ["10.0.0.5", "10.0.0.5:2828", "evil.example.com", "evil.example.com:2828", "[fe80::1]:2828", "127.0.0.1.evil.com"] {
            assert!(!is_loopback_host(host), "{host}");
        }
    }
}
