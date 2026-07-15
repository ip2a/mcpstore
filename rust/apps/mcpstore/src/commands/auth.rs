use std::io::Read;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand, ValueEnum};
use mcpstore::{AuthFlow, AuthStatusView, AuthorizationStart, InstanceId, MCPStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::{Host, Url};

use crate::store_args::{build_store, StoreSourceArgs};
use crate::BoxErr;

const DEFAULT_CALLBACK_TIMEOUT_SECONDS: u64 = 300;
const MAX_CALLBACK_REQUEST_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug)]
pub struct JsonAuthError {
    message: String,
}

impl JsonAuthError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for JsonAuthError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        json!({
            "event": "error",
            "error": {
                "code": "auth_command_failed",
                "message": self.message,
            }
        })
        .fmt(formatter)
    }
}

impl std::error::Error for JsonAuthError {}

#[derive(Args)]
pub struct AuthOutputArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Human,
        help = "Output format: human or json (JSON is emitted as JSON Lines for multi-step flows)"
    )]
    pub output: OutputFormat,
}

#[derive(Args)]
pub struct AuthFlowOutputArgs {
    #[command(flatten)]
    pub output: AuthOutputArgs,
    #[arg(
        long,
        help = "Do not open a browser automatically; print the authorization handoff and wait for the callback"
    )]
    pub non_interactive: bool,
}

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Subcommand)]
pub enum AuthAction {
    Status(AuthInstanceArgs),
    Login(AuthLoginArgs),
    Refresh(AuthInstanceArgs),
    Logout(AuthInstanceArgs),
    ScopeUpgrade(AuthScopeUpgradeArgs),
    SetClientSecret(AuthInstanceArgs),
    SetPrivateKey(AuthPrivateKeyArgs),
}

impl AuthAction {
    fn output_format(&self) -> OutputFormat {
        match self {
            Self::Status(args)
            | Self::Refresh(args)
            | Self::Logout(args)
            | Self::SetClientSecret(args) => args.output.output,
            Self::Login(args) => args.flow_output.output.output,
            Self::ScopeUpgrade(args) => args.flow_output.output.output,
            Self::SetPrivateKey(args) => args.output.output,
        }
    }
}

#[derive(Args)]
pub struct AuthInstanceArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[command(flatten)]
    pub output: AuthOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct AuthLoginArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[arg(
        long,
        default_value_t = DEFAULT_CALLBACK_TIMEOUT_SECONDS,
        help = "Local OAuth callback timeout in seconds"
    )]
    pub timeout: u64,
    #[command(flatten)]
    pub flow_output: AuthFlowOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct AuthScopeUpgradeArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[arg(long, help = "Space-delimited scope requested by the MCP server")]
    pub scope: Option<String>,
    #[arg(
        long,
        default_value_t = DEFAULT_CALLBACK_TIMEOUT_SECONDS,
        help = "Local OAuth callback timeout in seconds"
    )]
    pub timeout: u64,
    #[command(flatten)]
    pub flow_output: AuthFlowOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct AuthPrivateKeyArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[arg(
        long,
        value_name = "PEM_FILE",
        help = "Read the private key from this file instead of stdin"
    )]
    pub file: Option<PathBuf>,
    #[command(flatten)]
    pub output: AuthOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Debug, Deserialize)]
struct AuthStatusResponse {
    auth: AuthStatusView,
}

#[derive(Debug, Deserialize)]
struct AuthStartResponse {
    auth: AuthStatusView,
    authorization: Option<AuthorizationStart>,
}

#[derive(Debug, Deserialize)]
struct CallbackUriResponse {
    callback_uri: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum AuthOutputEvent<'a> {
    Status {
        auth: &'a AuthStatusView,
    },
    AuthorizationRequired {
        instance_id: &'a InstanceId,
        authorization_url: &'a str,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        scopes: &'a Vec<String>,
    },
    CredentialStored {
        credential: &'a str,
        stored: bool,
    },
}

#[derive(Debug)]
struct OAuthCallback {
    code: String,
    state: String,
    issuer: Option<String>,
}

#[derive(Debug)]
struct PendingOAuthCallback {
    callback: OAuthCallback,
    stream: tokio::net::TcpStream,
}

struct LocalCallbackListener {
    listener: TcpListener,
    callback_uri: Url,
}

pub async fn run(args: AuthArgs) -> Result<(), BoxErr> {
    let output = args.action.output_format();
    let result = match args.action {
        AuthAction::Status(args) => status(args).await,
        AuthAction::Login(args) => login(args).await,
        AuthAction::Refresh(args) => refresh(args).await,
        AuthAction::Logout(args) => logout(args).await,
        AuthAction::ScopeUpgrade(args) => scope_upgrade(args).await,
        AuthAction::SetClientSecret(args) => set_client_secret(args).await,
        AuthAction::SetPrivateKey(args) => set_private_key(args).await,
    };
    match (output, result) {
        (OutputFormat::Json, Err(error)) => Err(Box::new(JsonAuthError::new(error.to_string()))),
        (_, result) => result,
    }
}

async fn status(args: AuthInstanceArgs) -> Result<(), BoxErr> {
    let output = args.output.output;
    let auth = if crate::daemon::client::daemon_socket_exists() {
        daemon_auth_status(args.instance_id).await?
    } else {
        let store = loaded_store(&args.store).await?;
        store.auth_status_view(args.instance_id).await?
    };
    print_auth_status(&auth, output)
}

async fn login(args: AuthLoginArgs) -> Result<(), BoxErr> {
    let output = args.flow_output.output.output;
    let open_browser = !args.flow_output.non_interactive;
    if crate::daemon::client::daemon_socket_exists() {
        let auth = daemon_auth_status(args.instance_id).await?;
        match auth.flow {
            Some(AuthFlow::AuthorizationCode) => {
                let callback_uri = daemon_callback_uri(args.instance_id).await?;
                let listener = LocalCallbackListener::bind(&callback_uri).await?;
                let started: AuthStartResponse =
                    daemon_call("auth_begin", args.instance_id, json!({})).await?;
                let authorization = started
                    .authorization
                    .ok_or("Authorization server did not return an authorization URL")?;
                complete_daemon_browser_flow(
                    args.instance_id,
                    listener,
                    authorization,
                    args.timeout,
                    output,
                    open_browser,
                )
                .await
            }
            Some(AuthFlow::ClientCredentials) => {
                let response: AuthStartResponse =
                    daemon_call("auth_begin", args.instance_id, json!({})).await?;
                print_auth_status(&response.auth, output)
            }
            None => Err("Authentication is not configured for this instance".into()),
        }
    } else {
        let store = loaded_store(&args.store).await?;
        let auth = store.auth_status_view(args.instance_id).await?;
        match auth.flow {
            Some(AuthFlow::AuthorizationCode) => {
                let callback_uri = store
                    .authorization_callback_uri(args.instance_id)
                    .await?
                    .ok_or("Authorization Code flow has no callback URI")?;
                let listener = LocalCallbackListener::bind(&callback_uri).await?;
                let authorization = store.begin_authorization(args.instance_id).await?;
                complete_local_browser_flow(
                    &store,
                    args.instance_id,
                    listener,
                    authorization,
                    args.timeout,
                    output,
                    open_browser,
                )
                .await
            }
            Some(AuthFlow::ClientCredentials) => {
                store.refresh_authorization(args.instance_id).await?;
                reconnect_authorized_service(&store, args.instance_id).await?;
                let auth = store.auth_status_view(args.instance_id).await?;
                print_auth_status(&auth, output)
            }
            None => Err("Authentication is not configured for this instance".into()),
        }
    }
}

async fn refresh(args: AuthInstanceArgs) -> Result<(), BoxErr> {
    let output = args.output.output;
    let auth = if crate::daemon::client::daemon_socket_exists() {
        let response: AuthStatusResponse =
            daemon_call("auth_refresh", args.instance_id, json!({})).await?;
        response.auth
    } else {
        let store = loaded_store(&args.store).await?;
        store.refresh_authorization(args.instance_id).await?;
        reconnect_authorized_service(&store, args.instance_id).await?;
        store.auth_status_view(args.instance_id).await?
    };
    print_auth_status(&auth, output)
}

async fn logout(args: AuthInstanceArgs) -> Result<(), BoxErr> {
    let output = args.output.output;
    let auth = if crate::daemon::client::daemon_socket_exists() {
        let response: AuthStatusResponse =
            daemon_call("auth_logout", args.instance_id, json!({})).await?;
        response.auth
    } else {
        let store = loaded_store(&args.store).await?;
        store.logout_authorization(args.instance_id).await?;
        store.auth_status_view(args.instance_id).await?
    };
    print_auth_status(&auth, output)
}

async fn scope_upgrade(args: AuthScopeUpgradeArgs) -> Result<(), BoxErr> {
    let output = args.flow_output.output.output;
    let open_browser = !args.flow_output.non_interactive;
    if crate::daemon::client::daemon_socket_exists() {
        let auth = daemon_auth_status(args.instance_id).await?;
        let required_scope = required_scope(args.scope, &auth)?;
        let callback_uri = daemon_callback_uri(args.instance_id).await?;
        let listener = LocalCallbackListener::bind(&callback_uri).await?;
        let started: AuthStartResponse = daemon_call(
            "auth_scope_upgrade",
            args.instance_id,
            json!({"required_scope": required_scope}),
        )
        .await?;
        let authorization = started
            .authorization
            .ok_or("Authorization server did not return an authorization URL")?;
        complete_daemon_browser_flow(
            args.instance_id,
            listener,
            authorization,
            args.timeout,
            output,
            open_browser,
        )
        .await
    } else {
        let store = loaded_store(&args.store).await?;
        let auth = store.auth_status_view(args.instance_id).await?;
        let required_scope = required_scope(args.scope, &auth)?;
        let callback_uri = store
            .authorization_callback_uri(args.instance_id)
            .await?
            .ok_or("Scope upgrade requires Authorization Code authentication")?;
        let listener = LocalCallbackListener::bind(&callback_uri).await?;
        let authorization = store
            .begin_scope_upgrade(args.instance_id, &required_scope)
            .await?;
        complete_local_browser_flow(
            &store,
            args.instance_id,
            listener,
            authorization,
            args.timeout,
            output,
            open_browser,
        )
        .await
    }
}

async fn set_client_secret(args: AuthInstanceArgs) -> Result<(), BoxErr> {
    let output = args.output.output;
    let secret = read_stdin_secret("client secret")?;
    if crate::daemon::client::daemon_socket_exists() {
        let _: Value = daemon_call(
            "auth_save_client_secret",
            args.instance_id,
            json!({"client_secret": secret}),
        )
        .await?;
    } else {
        let store = loaded_store(&args.store).await?;
        store
            .save_oauth_client_secret(args.instance_id, secret)
            .await?;
    }
    print_credential_stored(output, "client_secret")
}

async fn set_private_key(args: AuthPrivateKeyArgs) -> Result<(), BoxErr> {
    let output = args.output.output;
    let private_key = match args.file {
        Some(path) => std::fs::read(path)?,
        None => read_stdin_bytes("private key")?,
    };
    if crate::daemon::client::daemon_socket_exists() {
        let private_key =
            String::from_utf8(private_key).map_err(|_| "Private key must be UTF-8 PEM data")?;
        let _: Value = daemon_call(
            "auth_save_private_key",
            args.instance_id,
            json!({"private_key_pem": private_key}),
        )
        .await?;
    } else {
        let store = loaded_store(&args.store).await?;
        store
            .save_oauth_private_key(args.instance_id, private_key)
            .await?;
    }
    print_credential_stored(output, "private_key")
}

async fn loaded_store(args: &StoreSourceArgs) -> Result<MCPStore, BoxErr> {
    let store = build_store(args)?;
    store.load_from_source().await?;
    Ok(store)
}

async fn daemon_auth_status(instance_id: InstanceId) -> Result<AuthStatusView, BoxErr> {
    let response: AuthStatusResponse = daemon_call("auth_status", instance_id, json!({})).await?;
    Ok(response.auth)
}

async fn daemon_callback_uri(instance_id: InstanceId) -> Result<String, BoxErr> {
    let response: CallbackUriResponse =
        daemon_call("auth_callback_uri", instance_id, json!({})).await?;
    response
        .callback_uri
        .ok_or_else(|| "Authorization Code flow has no callback URI".into())
}

async fn daemon_call<T: serde::de::DeserializeOwned>(
    method: &str,
    instance_id: InstanceId,
    extra: Value,
) -> Result<T, BoxErr> {
    let mut params = match extra {
        Value::Object(params) => params,
        _ => return Err("Daemon auth parameters must be an object".into()),
    };
    params.insert("instance_id".to_string(), json!(instance_id));
    let value = crate::daemon::client::call_daemon(method, Value::Object(params)).await?;
    Ok(serde_json::from_value(value)?)
}

async fn complete_daemon_browser_flow(
    instance_id: InstanceId,
    listener: LocalCallbackListener,
    authorization: AuthorizationStart,
    timeout_seconds: u64,
    output: OutputFormat,
    open_browser: bool,
) -> Result<(), BoxErr> {
    announce_authorization(&authorization, output, open_browser)?;
    let mut pending = listener.wait(timeout_seconds).await?;
    let result: Result<AuthStatusResponse, BoxErr> = daemon_call(
        "auth_callback",
        instance_id,
        json!({
            "code": pending.callback.code,
            "state": pending.callback.state,
            "issuer": pending.callback.issuer,
        }),
    )
    .await;
    write_browser_response(&mut pending.stream, result.is_ok()).await?;
    let response = result?;
    print_auth_status(&response.auth, output)
}

async fn complete_local_browser_flow(
    store: &MCPStore,
    instance_id: InstanceId,
    listener: LocalCallbackListener,
    authorization: AuthorizationStart,
    timeout_seconds: u64,
    output: OutputFormat,
    open_browser: bool,
) -> Result<(), BoxErr> {
    announce_authorization(&authorization, output, open_browser)?;
    let mut pending = listener.wait(timeout_seconds).await?;
    let result = async {
        store
            .complete_authorization_callback(
                instance_id,
                &pending.callback.code,
                &pending.callback.state,
                pending.callback.issuer.as_deref(),
            )
            .await?;
        reconnect_authorized_service(store, instance_id).await?;
        store.auth_status_view(instance_id).await
    }
    .await;
    write_browser_response(&mut pending.stream, result.is_ok()).await?;
    let auth = result?;
    print_auth_status(&auth, output)
}

async fn reconnect_authorized_service(
    store: &MCPStore,
    instance_id: InstanceId,
) -> mcpstore::Result<()> {
    store.disconnect_service(instance_id).await.ok();
    store.connect_service(instance_id).await
}

fn required_scope(requested: Option<String>, auth: &AuthStatusView) -> Result<String, BoxErr> {
    requested
        .or_else(|| auth.required_scope.clone())
        .filter(|scope| !scope.trim().is_empty())
        .map(|scope| scope.trim().to_string())
        .ok_or_else(|| "No required scope is pending; provide --scope".into())
}

fn announce_authorization(
    authorization: &AuthorizationStart,
    output: OutputFormat,
    open_browser: bool,
) -> Result<(), BoxErr> {
    match output {
        OutputFormat::Human => {
            println!("Open this URL to authorize MCPStore:");
            println!("{}", authorization.authorization_url);
            if open_browser && !try_open_browser(&authorization.authorization_url) {
                println!("The browser could not be opened automatically; use the URL above.");
            }
        }
        OutputFormat::Json => {
            // The authorization URL is an opaque provider handoff. The OAuth state,
            // authorization code, and tokens are never emitted as separate fields.
            print_json_event(&AuthOutputEvent::AuthorizationRequired {
                instance_id: &authorization.instance_id,
                authorization_url: &authorization.authorization_url,
                scopes: &authorization.scopes,
            })?;
            if open_browser {
                let _ = try_open_browser(&authorization.authorization_url);
            }
        }
    }
    Ok(())
}

fn try_open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "start", "", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xdg-open", vec![url]);

    std::process::Command::new(command.0)
        .args(command.1)
        .spawn()
        .is_ok()
}

fn print_auth_status(auth: &AuthStatusView, output: OutputFormat) -> Result<(), BoxErr> {
    match output {
        OutputFormat::Human => {
            println!("instance: {}", auth.instance_id);
            println!("status: {}", auth_status_name(auth));
            if let Some(flow) = auth.flow {
                println!(
                    "flow: {}",
                    match flow {
                        AuthFlow::AuthorizationCode => "authorization_code",
                        AuthFlow::ClientCredentials => "client_credentials",
                    }
                );
            }
            if !auth.scopes.is_empty() {
                println!("scopes: {}", auth.scopes.join(" "));
            }
            if let Some(scope) = &auth.required_scope {
                println!("required_scope: {scope}");
            }
            Ok(())
        }
        OutputFormat::Json => print_json_event(&AuthOutputEvent::Status { auth }),
    }
}

fn print_credential_stored(output: OutputFormat, credential: &str) -> Result<(), BoxErr> {
    match output {
        OutputFormat::Human => {
            println!("OAuth {credential} stored securely.");
            Ok(())
        }
        OutputFormat::Json => print_json_event(&AuthOutputEvent::CredentialStored {
            credential,
            stored: true,
        }),
    }
}

fn print_json_event(event: &AuthOutputEvent<'_>) -> Result<(), BoxErr> {
    println!("{}", serde_json::to_string(event)?);
    Ok(())
}

fn auth_status_name(auth: &AuthStatusView) -> &'static str {
    use mcpstore::AuthStatus;
    match auth.status {
        AuthStatus::NotRequired => "not_required",
        AuthStatus::Unauthenticated => "unauthenticated",
        AuthStatus::Authorizing => "authorizing",
        AuthStatus::Authenticated => "authenticated",
        AuthStatus::Refreshing => "refreshing",
        AuthStatus::ScopeUpgradeRequired => "scope_upgrade_required",
        AuthStatus::Error => "error",
    }
}

fn read_stdin_secret(kind: &str) -> Result<String, BoxErr> {
    let bytes = read_stdin_bytes(kind)?;
    let value = String::from_utf8(bytes).map_err(|_| format!("{kind} must be UTF-8"))?;
    let value = value.trim_end_matches(['\r', '\n']).to_string();
    if value.is_empty() {
        return Err(format!("{kind} must not be empty").into());
    }
    Ok(value)
}

fn read_stdin_bytes(kind: &str) -> Result<Vec<u8>, BoxErr> {
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        return Err(format!("{kind} must not be empty").into());
    }
    Ok(bytes)
}

impl LocalCallbackListener {
    async fn bind(callback_uri: &str) -> Result<Self, BoxErr> {
        let callback_uri = Url::parse(callback_uri)?;
        if callback_uri.scheme() != "http" {
            return Err("OAuth CLI callback URI must use http on a loopback host".into());
        }
        if callback_uri.fragment().is_some() {
            return Err("OAuth CLI callback URI must not contain a fragment".into());
        }
        let host = callback_uri
            .host()
            .ok_or("OAuth CLI callback URI must include a loopback host")?;
        if !is_loopback_host(&host) {
            return Err(
                "OAuth CLI callback URI must use localhost or a loopback IP address".into(),
            );
        }
        let host_text = callback_uri
            .host_str()
            .ok_or("OAuth CLI callback URI must include a host")?;
        let port = callback_uri
            .port_or_known_default()
            .ok_or("OAuth CLI callback URI must include a port")?;
        if port == 0 {
            return Err("OAuth CLI callback URI port must not be zero".into());
        }
        let address = match host {
            Host::Ipv6(_) => format!("[{host_text}]:{port}"),
            _ => format!("{host_text}:{port}"),
        };
        let listener = TcpListener::bind(address).await?;
        Ok(Self {
            listener,
            callback_uri,
        })
    }

    async fn wait(self, timeout_seconds: u64) -> Result<PendingOAuthCallback, BoxErr> {
        let timeout = Duration::from_secs(timeout_seconds.max(1));
        tokio::time::timeout(timeout, self.wait_for_callback())
            .await
            .map_err(|_| -> BoxErr { "OAuth callback timed out".into() })?
    }

    async fn wait_for_callback(self) -> Result<PendingOAuthCallback, BoxErr> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let request = read_http_request(&mut stream).await;
            let callback = request
                .as_deref()
                .ok()
                .and_then(|request| parse_callback_request(&self.callback_uri, request).ok());
            if let Some(callback) = callback {
                return Ok(PendingOAuthCallback { callback, stream });
            }
            write_browser_response(&mut stream, false).await?;
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

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> Result<String, BoxErr> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
        if request.len() > MAX_CALLBACK_REQUEST_BYTES {
            return Err("OAuth callback request is too large".into());
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8(request)?)
}

fn parse_callback_request(callback_uri: &Url, request: &str) -> Result<OAuthCallback, BoxErr> {
    let request_line = request.lines().next().ok_or("Missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err("OAuth callback must use GET".into());
    }
    let target = parts
        .next()
        .ok_or("OAuth callback request target is missing")?;
    let target = Url::parse(&format!("http://localhost{target}"))?;
    if target.path() != callback_uri.path() {
        return Err("OAuth callback path does not match the configured redirect URI".into());
    }
    let target_query: Vec<_> = target.query_pairs().collect();
    if callback_uri
        .query_pairs()
        .any(|expected| !target_query.iter().any(|actual| actual == &expected))
    {
        return Err("OAuth callback query does not match the configured redirect URI".into());
    }

    let mut code = None;
    let mut state = None;
    let mut issuer = None;
    for (key, value) in target.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "iss" => issuer = Some(value.into_owned()),
            _ => {}
        }
    }
    Ok(OAuthCallback {
        code: code
            .filter(|value| !value.is_empty())
            .ok_or("OAuth callback is missing code")?,
        state: state
            .filter(|value| !value.is_empty())
            .ok_or("OAuth callback is missing state")?,
        issuer,
    })
}

async fn write_browser_response(
    stream: &mut tokio::net::TcpStream,
    success: bool,
) -> Result<(), BoxErr> {
    let (status, title, message) = if success {
        (
            "200 OK",
            "Authorization complete",
            "You can close this window and return to MCPStore.",
        )
    } else {
        (
            "404 Not Found",
            "Callback not accepted",
            "This is not the expected MCPStore OAuth callback.",
        )
    };
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body><h1>{title}</h1><p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn json_auth_error_is_a_stable_non_sensitive_event() {
        let error = JsonAuthError::new("OAuth callback was rejected");
        let value: Value = serde_json::from_str(&error.to_string()).unwrap();

        assert_eq!(value["event"], "error");
        assert_eq!(value["error"]["code"], "auth_command_failed");
        assert_eq!(value["error"]["message"], "OAuth callback was rejected");
        for forbidden in [
            "access_token",
            "refresh_token",
            "client_secret",
            "private_key",
            "authorization_code",
            "state",
        ] {
            assert!(value.get(forbidden).is_none());
            assert!(value["error"].get(forbidden).is_none());
        }
    }

    #[test]
    fn json_status_event_contains_only_non_sensitive_auth_state() {
        let auth = AuthStatusView {
            instance_id: "c81af510-755b-55c7-8487-5668ab36e06e".parse().unwrap(),
            status: mcpstore::AuthStatus::Authenticated,
            flow: Some(AuthFlow::AuthorizationCode),
            scopes: vec!["read".to_string()],
            required_scope: None,
        };
        let value = serde_json::to_value(AuthOutputEvent::Status { auth: &auth }).unwrap();

        assert_eq!(value["event"], "status");
        assert_eq!(value["auth"]["status"], "authenticated");
        for forbidden in [
            "access_token",
            "refresh_token",
            "client_secret",
            "private_key",
            "authorization_code",
            "state",
        ] {
            assert!(value.get(forbidden).is_none());
            assert!(value["auth"].get(forbidden).is_none());
        }
    }

    #[test]
    fn authorization_json_event_keeps_url_opaque_and_does_not_emit_callback_fields() {
        let authorization = AuthorizationStart {
            instance_id: "c81af510-755b-55c7-8487-5668ab36e06e".parse().unwrap(),
            authorization_url: "https://issuer.example/authorize?state=opaque-state".to_string(),
            scopes: vec!["read".to_string()],
        };
        let value = serde_json::to_value(AuthOutputEvent::AuthorizationRequired {
            instance_id: &authorization.instance_id,
            authorization_url: &authorization.authorization_url,
            scopes: &authorization.scopes,
        })
        .unwrap();

        assert_eq!(value["event"], "authorization_required");
        assert_eq!(value["authorization_url"], authorization.authorization_url);
        assert!(value.get("state").is_none());
        assert!(value.get("code").is_none());
    }

    async fn listener_fixture(path: &str) -> (LocalCallbackListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        (
            LocalCallbackListener {
                listener,
                callback_uri: Url::parse(&format!("http://{address}{path}")).unwrap(),
            },
            address.to_string(),
        )
    }

    #[tokio::test]
    async fn callback_listener_accepts_expected_path_and_keeps_secrets_out_of_browser_response() {
        let (listener, address) = listener_fixture("/oauth/callback").await;
        let client = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream
                .write_all(b"GET /oauth/callback?code=secret-code&state=secret-state&iss=https%3A%2F%2Fissuer.example HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            response
        });

        let mut pending = listener.wait(2).await.unwrap();
        write_browser_response(&mut pending.stream, true)
            .await
            .unwrap();
        let response = client.await.unwrap();
        assert_eq!(pending.callback.code, "secret-code");
        assert_eq!(pending.callback.state, "secret-state");
        assert_eq!(
            pending.callback.issuer.as_deref(),
            Some("https://issuer.example")
        );
        assert!(response.contains("Authorization complete"));
        assert!(!response.contains("secret-code"));
        assert!(!response.contains("secret-state"));
    }

    #[tokio::test]
    async fn callback_listener_ignores_wrong_path_then_accepts_configured_path() {
        let (listener, address) = listener_fixture("/oauth/callback").await;
        let client = tokio::spawn(async move {
            let mut wrong = tokio::net::TcpStream::connect(&address).await.unwrap();
            wrong
                .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            let mut wrong_response = String::new();
            wrong.read_to_string(&mut wrong_response).await.unwrap();
            assert!(wrong_response.starts_with("HTTP/1.1 404"));

            let mut callback_stream = tokio::net::TcpStream::connect(address).await.unwrap();
            callback_stream
                .write_all(b"GET /oauth/callback?code=code&state=state HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
        });
        let mut pending = listener.wait(2).await.unwrap();
        write_browser_response(&mut pending.stream, true)
            .await
            .unwrap();
        client.await.unwrap();
        assert_eq!(pending.callback.code, "code");
        assert_eq!(pending.callback.state, "state");
    }

    #[tokio::test]
    async fn callback_listener_times_out() {
        let (listener, _) = listener_fixture("/oauth/callback").await;
        let error = listener.wait(1).await.unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn callback_parser_leaves_state_validation_to_rmcp_session() {
        let callback_uri = Url::parse("http://127.0.0.1:8787/oauth/callback").unwrap();
        let callback = parse_callback_request(
            &callback_uri,
            "GET /oauth/callback?code=code&state=untrusted HTTP/1.1\r\n\r\n",
        )
        .unwrap();
        assert_eq!(callback.state, "untrusted");
    }

    #[test]
    fn callback_parser_requires_configured_query_parameters() {
        let callback_uri = Url::parse("http://127.0.0.1:8787/oauth/callback?channel=cli").unwrap();
        let missing = parse_callback_request(
            &callback_uri,
            "GET /oauth/callback?code=code&state=state HTTP/1.1\r\n\r\n",
        );
        assert!(missing
            .unwrap_err()
            .to_string()
            .contains("configured redirect URI"));

        let callback = parse_callback_request(
            &callback_uri,
            "GET /oauth/callback?channel=cli&code=code&state=state HTTP/1.1\r\n\r\n",
        )
        .unwrap();
        assert_eq!(callback.code, "code");
        assert_eq!(callback.state, "state");
    }

    #[test]
    fn non_loopback_callback_uri_is_rejected() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let error = runtime
            .block_on(LocalCallbackListener::bind(
                "http://example.com/oauth/callback",
            ))
            .err()
            .unwrap();
        assert!(error.to_string().contains("loopback"));
    }
}
