use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StartupPolicy {
    Manual,
    Lazy,
    OnStoreStart,
}

impl Default for StartupPolicy {
    fn default() -> Self {
        Self::Lazy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicyKind {
    No,
    OnFailure,
    Always,
    UnlessStopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartPolicy {
    pub kind: RestartPolicyKind,
    pub max_retries: Option<i32>,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            kind: RestartPolicyKind::No,
            max_retries: None,
        }
    }
}

impl RestartPolicy {
    pub fn should_restart_after_failure(&self, attempts: i32) -> bool {
        match self.kind {
            RestartPolicyKind::No => false,
            RestartPolicyKind::OnFailure => {
                self.max_retries.map(|max| attempts <= max).unwrap_or(true)
            }
            RestartPolicyKind::Always | RestartPolicyKind::UnlessStopped => true,
        }
    }

    pub fn is_unless_stopped(&self) -> bool {
        self.kind == RestartPolicyKind::UnlessStopped
    }
}

impl Serialize for RestartPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match (self.kind.clone(), self.max_retries) {
            (RestartPolicyKind::No, _) => "no".to_string(),
            (RestartPolicyKind::OnFailure, Some(max)) => format!("on-failure:{max}"),
            (RestartPolicyKind::OnFailure, None) => "on-failure".to_string(),
            (RestartPolicyKind::Always, _) => "always".to_string(),
            (RestartPolicyKind::UnlessStopped, _) => "unless-stopped".to_string(),
        };
        serializer.serialize_str(&value)
    }
}

impl<'de> Deserialize<'de> for RestartPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_restart_policy(&value).map_err(serde::de::Error::custom)
    }
}

fn parse_restart_policy(value: &str) -> Result<RestartPolicy, String> {
    match value {
        "no" => Ok(RestartPolicy::default()),
        "on-failure" => Ok(RestartPolicy {
            kind: RestartPolicyKind::OnFailure,
            max_retries: None,
        }),
        "always" => Ok(RestartPolicy {
            kind: RestartPolicyKind::Always,
            max_retries: None,
        }),
        "unless-stopped" => Ok(RestartPolicy {
            kind: RestartPolicyKind::UnlessStopped,
            max_retries: None,
        }),
        _ if value.starts_with("on-failure:") => {
            let max = value
                .trim_start_matches("on-failure:")
                .parse::<i32>()
                .map_err(|_| format!("Invalid restart_policy value: {value}"))?;
            if max < 0 {
                return Err(format!("Invalid restart_policy value: {value}"));
            }
            Ok(RestartPolicy {
                kind: RestartPolicyKind::OnFailure,
                max_retries: Some(max),
            })
        }
        _ => Err(format!("Invalid restart_policy value: {value}")),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceLifecycleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_policy: Option<StartupPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceLifecycleDefaults {
    #[serde(default)]
    pub startup_policy: StartupPolicy,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
}

impl Default for ServiceLifecycleDefaults {
    fn default() -> Self {
        Self {
            startup_policy: StartupPolicy::Lazy,
            restart_policy: RestartPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedServiceLifecycle {
    pub startup_policy: StartupPolicy,
    pub restart_policy: RestartPolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpStoreExtension {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ServiceLifecycleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(rename = "workingDir", default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "_mcpstore", default, skip_serializing_if = "Option::is_none")]
    pub mcpstore: Option<McpStoreExtension>,
}

impl ServerConfig {
    pub fn infer_transport(&self) -> &str {
        if let Some(ref transport) = self.transport {
            return transport.as_str();
        }
        if self.url.is_some() {
            "streamable-http"
        } else if self.command.is_some() {
            "stdio"
        } else {
            "unknown"
        }
    }

    pub fn resolved_lifecycle(
        &self,
        defaults: &ServiceLifecycleDefaults,
    ) -> ResolvedServiceLifecycle {
        let lifecycle = self
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.as_ref());
        ResolvedServiceLifecycle {
            startup_policy: lifecycle
                .and_then(|value| value.startup_policy.clone())
                .unwrap_or_else(|| defaults.startup_policy.clone()),
            restart_policy: lifecycle
                .and_then(|value| value.restart_policy.clone())
                .unwrap_or_else(|| defaults.restart_policy.clone()),
        }
    }
}
