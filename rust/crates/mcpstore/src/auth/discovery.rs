use std::collections::HashMap;
use std::time::Duration;

use http::header::WWW_AUTHENTICATE;

/// Returns true when the MCP HTTP endpoint responds with 401 and a Bearer challenge.
/// Configured headers (e.g. an Authorization bearer token) are sent with the probe so
/// header-authenticated endpoints with valid credentials are not misread as OAuth-only.
pub async fn http_endpoint_requires_oauth(url: &str, headers: &HashMap<String, String>) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    else {
        return false;
    };

    let mut request = client
        .post(url)
        .header(http::header::ACCEPT, "application/json")
        .header(http::header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = match request.body("{}").send().await {
        Ok(response) => response,
        Err(_) => return false,
    };

    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return false;
    }

    has_bearer_challenge(response.headers())
}

fn has_bearer_challenge(headers: &http::HeaderMap) -> bool {
    headers
        .get_all(WWW_AUTHENTICATE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.to_ascii_lowercase().contains("bearer"))
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use std::collections::HashMap;

    use super::{has_bearer_challenge, http_endpoint_requires_oauth};

    #[test]
    fn bearer_challenge_is_required_for_oauth_detection() {
        let mut headers = HeaderMap::new();
        assert!(!has_bearer_challenge(&headers));

        headers.insert(
            http::header::WWW_AUTHENTICATE,
            "Basic realm=\"mcp\"".parse().unwrap(),
        );
        assert!(!has_bearer_challenge(&headers));

        headers.append(
            http::header::WWW_AUTHENTICATE,
            "Bearer resource_metadata=\"https://example.com/.well-known/oauth-protected-resource\""
                .parse()
                .unwrap(),
        );
        assert!(has_bearer_challenge(&headers));
    }

    #[tokio::test]
    async fn protected_http_endpoint_is_detected_from_its_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            stream.read(&mut request).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer realm=\"mcp\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });

        assert!(
            http_endpoint_requires_oauth(&format!("http://{address}/mcp"), &HashMap::new()).await
        );
    }
}
