use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::auth::AuthConfig;
use crate::identity::ScopeRef;

use super::merge_config;

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
    pub scopes: ScopeDeclarations,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ServiceLifecycleConfig>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeDeclarations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<ScopeDescriptor>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<String, ScopeDescriptor>,
}

impl ScopeDeclarations {
    pub fn store_only() -> Self {
        Self {
            store: Some(ScopeDescriptor::default()),
            agents: HashMap::new(),
        }
    }

    pub fn descriptor(&self, scope: &ScopeRef) -> Option<&ScopeDescriptor> {
        match scope {
            ScopeRef::Store => self.store.as_ref(),
            ScopeRef::Agent { agent_id } => self.agents.get(agent_id),
        }
    }

    pub fn scopes(&self) -> Vec<ScopeRef> {
        let mut scopes = Vec::with_capacity(self.agents.len() + usize::from(self.store.is_some()));
        if self.store.is_some() {
            scopes.push(ScopeRef::Store);
        }
        let mut agent_ids = self.agents.keys().cloned().collect::<Vec<_>>();
        agent_ids.sort();
        scopes.extend(
            agent_ids
                .into_iter()
                .map(|agent_id| ScopeRef::Agent { agent_id }),
        );
        scopes
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeDescriptor {
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub config: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ServiceLifecycleConfig>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "AuthConfig::is_none")]
    pub auth: AuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(
        rename = "workingDir",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub working_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "_mcpstore", default, skip_serializing_if = "Option::is_none")]
    pub mcpstore: Option<McpStoreExtension>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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
        self.resolved_lifecycle_for_scope(&ScopeRef::Store, defaults)
    }

    pub fn resolved_lifecycle_for_scope(
        &self,
        scope: &ScopeRef,
        defaults: &ServiceLifecycleDefaults,
    ) -> ResolvedServiceLifecycle {
        let definition_lifecycle = self
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.as_ref());
        let scopes = self.scopes();
        let scope_lifecycle = scopes
            .descriptor(scope)
            .and_then(|descriptor| descriptor.lifecycle.as_ref());
        ResolvedServiceLifecycle {
            startup_policy: scope_lifecycle
                .and_then(|value| value.startup_policy.clone())
                .or_else(|| definition_lifecycle.and_then(|value| value.startup_policy.clone()))
                .unwrap_or_else(|| defaults.startup_policy.clone()),
            restart_policy: scope_lifecycle
                .and_then(|value| value.restart_policy.clone())
                .or_else(|| definition_lifecycle.and_then(|value| value.restart_policy.clone()))
                .unwrap_or_else(|| defaults.restart_policy.clone()),
        }
    }

    pub fn scopes(&self) -> ScopeDeclarations {
        self.mcpstore
            .as_ref()
            .map(|extension| extension.scopes.clone())
            .unwrap_or_else(ScopeDeclarations::store_only)
    }

    pub fn base_config(&self) -> Map<String, Value> {
        let mut value =
            serde_json::to_value(self).expect("ServerConfig serialization must succeed");
        let object = value
            .as_object_mut()
            .expect("ServerConfig must serialize as a JSON object");
        object.remove("_mcpstore");
        object.clone()
    }

    pub fn effective_config(&self, scope: &ScopeRef) -> Result<Map<String, Value>, String> {
        let scopes = self.scopes();
        let descriptor = scopes
            .descriptor(scope)
            .ok_or_else(|| format!("scope {scope:?} is not declared"))?;
        let effective = merge_config(&self.base_config(), &descriptor.config);
        serde_json::from_value::<Self>(Value::Object(effective.clone()))
            .map_err(|error| format!("effective config cannot be decoded: {error}"))?;
        Ok(effective)
    }

    pub fn transport_config(&self, scope: &ScopeRef) -> Result<Self, String> {
        let effective = self.effective_config(scope)?;
        let mut config: Self = serde_json::from_value(Value::Object(effective))
            .map_err(|error| format!("effective config cannot be decoded: {error}"))?;
        config.mcpstore = None;
        Ok(config)
    }

    pub fn validate_structure(&self) -> Result<(), String> {
        let scopes = self.scopes();
        for agent_id in scopes.agents.keys() {
            if agent_id.trim().is_empty() {
                return Err("scopes.agents contains an empty agent id".to_string());
            }
        }
        Ok(())
    }

    pub fn definition_revision(&self) -> u64 {
        self.mcpstore
            .as_ref()
            .map(|extension| extension.revision.max(1))
            .unwrap_or(1)
    }

    pub fn scope_revision(&self, scope: &ScopeRef) -> Option<u64> {
        self.scopes()
            .descriptor(scope)
            .map(|descriptor| descriptor.revision.max(1))
    }

    pub fn ensure_native_scopes(&mut self) {
        if self.mcpstore.is_none() {
            self.mcpstore = Some(McpStoreExtension {
                scopes: ScopeDeclarations::store_only(),
                lifecycle: None,
                revision: 1,
                extra: Map::new(),
            });
        }
    }
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}
