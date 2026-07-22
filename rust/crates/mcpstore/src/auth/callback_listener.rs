use std::net::IpAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::{Host, Url};

const MAX_CALLBACK_REQUEST_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
    pub issuer: Option<String>,
}

pub struct LocalCallbackListener {
    listener: TcpListener,
    callback_uri: Url,
}

impl LocalCallbackListener {
    pub async fn bind(callback_uri: &str) -> Result<Self, String> {
        let callback_uri = Url::parse(callback_uri).map_err(|error| error.to_string())?;
        if callback_uri.scheme() != "http" {
            return Err("OAuth callback URI must use http on a loopback host".to_string());
        }
        if callback_uri.fragment().is_some() {
            return Err("OAuth callback URI must not contain a fragment".to_string());
        }
        let host = callback_uri
            .host()
            .ok_or_else(|| "OAuth callback URI must include a loopback host".to_string())?;
        if !is_loopback_host(&host) {
            return Err(
                "OAuth callback URI must use localhost or a loopback IP address".to_string(),
            );
        }
        let host_text = callback_uri
            .host_str()
            .ok_or_else(|| "OAuth callback URI must include a host".to_string())?;
        let port = callback_uri
            .port_or_known_default()
            .ok_or_else(|| "OAuth callback URI must include a port".to_string())?;
        if port == 0 {
            return Err("OAuth callback URI port must not be zero".to_string());
        }
        let address = match host {
            Host::Ipv6(_) => format!("[{host_text}]:{port}"),
            _ => format!("{host_text}:{port}"),
        };
        let listener = TcpListener::bind(address)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            listener,
            callback_uri,
        })
    }

    pub async fn wait(self, timeout_seconds: u64) -> Result<OAuthCallback, String> {
        let timeout = Duration::from_secs(timeout_seconds.max(1));
        tokio::time::timeout(timeout, self.wait_for_callback())
            .await
            .map_err(|_| "OAuth callback timed out".to_string())?
    }

    async fn wait_for_callback(self) -> Result<OAuthCallback, String> {
        loop {
            let (mut stream, _) = self
                .listener
                .accept()
                .await
                .map_err(|error| error.to_string())?;
            let request = read_http_request(&mut stream).await?;
            match parse_callback_request(&self.callback_uri, &request) {
                Ok(callback) => {
                    write_browser_response(&mut stream, true).await?;
                    return Ok(callback);
                }
                Err(_) => {
                    write_browser_response(&mut stream, false).await?;
                }
            }
        }
    }
}

fn is_loopback_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => IpAddr::V4(*address).is_loopback(),
        Host::Ipv6(address) => IpAddr::V6(*address).is_loopback(),
    }
}

async fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
        if request.len() > MAX_CALLBACK_REQUEST_BYTES {
            return Err("OAuth callback request is too large".to_string());
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(request).map_err(|error| error.to_string())
}

fn parse_callback_request(callback_uri: &Url, request: &str) -> Result<OAuthCallback, String> {
    let request_line = request.lines().next().ok_or("Missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err("OAuth callback must use GET".to_string());
    }
    let target = parts
        .next()
        .ok_or("OAuth callback request target is missing")?;
    let target =
        Url::parse(&format!("http://localhost{target}")).map_err(|error| error.to_string())?;
    if target.path() != callback_uri.path() {
        return Err("OAuth callback path does not match the configured redirect URI".to_string());
    }
    let target_query: Vec<_> = target.query_pairs().collect();
    if callback_uri.query_pairs().any(|expected| {
        !target_query
            .iter()
            .any(|actual| actual.0 == expected.0 && actual.1 == expected.1)
    }) {
        return Err("OAuth callback query does not match the configured redirect URI".to_string());
    }

    let code = target
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
        .ok_or("OAuth callback is missing code".to_string())?;
    let state = target
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
        .ok_or("OAuth callback is missing state".to_string())?;
    let issuer = target
        .query_pairs()
        .find(|(key, _)| key == "iss")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty());

    Ok(OAuthCallback {
        code,
        state,
        issuer,
    })
}

async fn write_browser_response(stream: &mut TcpStream, success: bool) -> Result<(), String> {
    let (status, title, body) = if success {
        (
            "200 OK",
            "Authorization complete",
            "You can close this window and return to MCPStore.",
        )
    } else {
        (
            "400 Bad Request",
            "Authorization failed",
            "The OAuth callback could not be processed. Return to MCPStore and try again.",
        )
    };
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body><p>{body}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{html}",
        html.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream.flush().await.map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_callback_request, LocalCallbackListener};
    use url::Url;

    #[test]
    fn callback_parser_requires_matching_path_code_and_state() {
        let callback_uri = Url::parse("http://127.0.0.1:8787/oauth/callback").unwrap();
        let callback = parse_callback_request(
            &callback_uri,
            "GET /oauth/callback?code=abc&state=xyz&iss=https%3A%2F%2Fissuer.example HTTP/1.1\r\n\r\n",
        )
        .unwrap();
        assert_eq!(callback.code, "abc");
        assert_eq!(callback.state, "xyz");
        assert_eq!(callback.issuer.as_deref(), Some("https://issuer.example"));

        assert!(parse_callback_request(
            &callback_uri,
            "GET /wrong?code=abc&state=xyz HTTP/1.1\r\n\r\n",
        )
        .is_err());
        assert!(parse_callback_request(
            &callback_uri,
            "GET /oauth/callback?code=abc HTTP/1.1\r\n\r\n",
        )
        .is_err());
    }

    #[tokio::test]
    async fn callback_listener_rejects_non_loopback_redirects() {
        assert!(
            LocalCallbackListener::bind("https://example.com/oauth/callback")
                .await
                .is_err()
        );
        assert!(
            LocalCallbackListener::bind("http://example.com:8787/oauth/callback")
                .await
                .is_err()
        );
    }
}
