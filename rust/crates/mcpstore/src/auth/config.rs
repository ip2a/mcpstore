use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

/// Default OAuth redirect URI for browser login flows (CLI and API listener).
pub const DEFAULT_OAUTH_REDIRECT_URI: &str = "http://127.0.0.1:8787/oauth/callback";

pub fn default_oauth_redirect_uri() -> String {
    DEFAULT_OAUTH_REDIRECT_URI.to_string()
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    #[default]
    None,
    #[serde(rename = "oauth_authorization_code")]
    OAuthAuthorizationCode(OAuthAuthorizationCodeConfig),
    #[serde(rename = "oauth_client_credentials")]
    OAuthClientCredentials(OAuthClientCredentialsConfig),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum AuthConfigInput {
    None,
    #[serde(rename = "oauth_authorization_code")]
    OAuthAuthorizationCode(OAuthAuthorizationCodeConfig),
    #[serde(rename = "oauth_client_credentials")]
    OAuthClientCredentials(OAuthClientCredentialsConfig),
}

impl<'de> Deserialize<'de> for AuthConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let config = match AuthConfigInput::deserialize(deserializer)? {
            AuthConfigInput::None => Self::None,
            AuthConfigInput::OAuthAuthorizationCode(mut config) => {
                config.normalize();
                Self::OAuthAuthorizationCode(config)
            }
            AuthConfigInput::OAuthClientCredentials(config) => Self::OAuthClientCredentials(config),
        };
        config.validate().map_err(serde::de::Error::custom)?;
        Ok(config)
    }
}

impl AuthConfig {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn scopes(&self) -> &[String] {
        match self {
            Self::None => &[],
            Self::OAuthAuthorizationCode(config) => &config.scopes,
            Self::OAuthClientCredentials(config) => &config.scopes,
        }
    }

    pub fn resource(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::OAuthAuthorizationCode(config) => config.resource.as_deref(),
            Self::OAuthClientCredentials(config) => config.resource.as_deref(),
        }
    }

    pub fn audience(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::OAuthAuthorizationCode(config) => config.audience.as_deref(),
            Self::OAuthClientCredentials(config) => config.audience.as_deref(),
        }
    }

    pub fn credential_profile(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::OAuthAuthorizationCode(config) => config.credential_profile.as_deref(),
            Self::OAuthClientCredentials(config) => config.credential_profile.as_deref(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::None => Ok(()),
            Self::OAuthAuthorizationCode(config) => {
                let mut config = config.clone();
                config.normalize();
                require_non_empty("auth.redirect_uri", &config.redirect_uri)?;
                match (&config.client_id, &config.client_metadata_url) {
                    (Some(client_id), None) => require_non_empty("auth.client_id", client_id)?,
                    (None, Some(client_metadata_url)) => {
                        validate_client_metadata_url(client_metadata_url)?;
                        if !matches!(
                            config.client_auth_method,
                            AuthorizationCodeClientAuthMethod::None
                        ) {
                            return Err(
                                "auth.client_auth_method requires a pre-registered auth.client_id"
                                    .to_string(),
                            );
                        }
                    }
                    (Some(_), Some(_)) => {
                        return Err(
                            "configure exactly one of auth.client_id or auth.client_metadata_url"
                                .to_string(),
                        );
                    }
                    (None, None) => {
                        return Err(
                            "configure auth.client_id or auth.client_metadata_url".to_string()
                        );
                    }
                }
                validate_common_fields(
                    &config.scopes,
                    config.resource.as_deref(),
                    config.audience.as_deref(),
                    config.credential_profile.as_deref(),
                )
            }
            Self::OAuthClientCredentials(config) => {
                require_non_empty("auth.client_id", &config.client_id)?;
                validate_common_fields(
                    &config.scopes,
                    config.resource.as_deref(),
                    config.audience.as_deref(),
                    config.credential_profile.as_deref(),
                )
            }
        }
    }
}

fn validate_common_fields(
    scopes: &[String],
    resource: Option<&str>,
    audience: Option<&str>,
    credential_profile: Option<&str>,
) -> Result<(), String> {
    for scope in scopes {
        require_non_empty("auth.scopes", scope)?;
    }
    for (field, value) in [
        ("auth.resource", resource),
        ("auth.audience", audience),
        ("auth.credential_profile", credential_profile),
    ] {
        if let Some(value) = value {
            require_non_empty(field, value)?;
        }
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

fn validate_client_metadata_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value)
        .map_err(|_| "auth.client_metadata_url must be a valid HTTPS URL".to_string())?;
    if url.scheme() != "https" || url.host_str().is_none() || url.path() == "/" {
        return Err(
            "auth.client_metadata_url must be an HTTPS URL with a non-root path".to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OAuthAuthorizationCodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_metadata_url: Option<String>,
    #[serde(default = "default_oauth_redirect_uri")]
    pub redirect_uri: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_profile: Option<String>,
    #[serde(default)]
    pub client_auth_method: AuthorizationCodeClientAuthMethod,
}

impl OAuthAuthorizationCodeConfig {
    pub fn normalize(&mut self) {
        if self.redirect_uri.trim().is_empty() {
            self.redirect_uri = default_oauth_redirect_uri();
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationCodeClientAuthMethod {
    #[default]
    None,
    ClientSecretBasic,
    ClientSecretPost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OAuthClientCredentialsConfig {
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_profile: Option<String>,
    #[serde(default)]
    pub client_auth_method: ClientCredentialsAuthMethod,
    #[serde(default)]
    pub jwt_signing_algorithm: JwtSigningAlgorithm,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientCredentialsAuthMethod {
    #[default]
    ClientSecretPost,
    PrivateKeyJwt,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JwtSigningAlgorithm {
    #[default]
    Rs256,
    Rs384,
    Rs512,
    Es256,
    Es384,
}
