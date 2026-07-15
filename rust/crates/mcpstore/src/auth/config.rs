use serde::{Deserialize, Deserializer, Serialize};

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
            AuthConfigInput::OAuthAuthorizationCode(config) => Self::OAuthAuthorizationCode(config),
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
                require_non_empty("auth.redirect_uri", &config.redirect_uri)?;
                if let Some(client_id) = &config.client_id {
                    require_non_empty("auth.client_id", client_id)?;
                } else if !config.dynamic_client_registration {
                    return Err(
                        "auth.client_id is required unless dynamic_client_registration is enabled"
                            .to_string(),
                    );
                } else if !matches!(
                    config.client_auth_method,
                    AuthorizationCodeClientAuthMethod::None
                ) {
                    return Err(
                        "auth.client_auth_method cannot be forced when dynamic_client_registration is enabled"
                            .to_string(),
                    );
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OAuthAuthorizationCodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
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
    pub dynamic_client_registration: bool,
    #[serde(default)]
    pub client_auth_method: AuthorizationCodeClientAuthMethod,
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
