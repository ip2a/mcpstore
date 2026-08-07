use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const INSTANCE_NAMESPACE: Uuid = Uuid::from_bytes([
    0xb6, 0xa7, 0xe6, 0xc8, 0x7d, 0x70, 0x5f, 0x9a, 0x9a, 0x8c, 0x3c, 0x93, 0xf7, 0xb9, 0x15, 0xf2,
]);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScopeRef {
    Store,
    Agent { agent_id: String },
}

impl ScopeRef {
    pub fn agent_id(&self) -> Option<&str> {
        match self {
            Self::Store => None,
            Self::Agent { agent_id } => Some(agent_id),
        }
    }

    fn canonical_parts(&self) -> (&'static str, &str) {
        match self {
            Self::Store => ("store", ""),
            Self::Agent { agent_id } => ("agent", agent_id),
        }
    }
}

/// 读视图 / 注册表作用域：在实例作用域 [`ScopeRef`] 之上加 `Root`。
///
/// `Root` 不是实例作用域——服务不会“声明在 root”，它只表示读聚合（store ∪ 所有 agent）。
/// 因此 `Root` 仅用于列表 / 注册表 / 读视图；声明类操作仍走 [`ScopeRef`]。
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScopeView {
    Root,
    Store,
    Agent { agent_id: String },
}

impl ScopeView {
    /// `Root` 没有对应的实例作用域；`Store` / `Agent` 返回对应 [`ScopeRef`]。
    pub fn as_scope_ref(&self) -> Option<ScopeRef> {
        match self {
            ScopeView::Root => None,
            ScopeView::Store => Some(ScopeRef::Store),
            ScopeView::Agent { agent_id } => Some(ScopeRef::Agent {
                agent_id: agent_id.clone(),
            }),
        }
    }
}

impl From<ScopeRef> for ScopeView {
    fn from(scope: ScopeRef) -> Self {
        match scope {
            ScopeRef::Store => ScopeView::Store,
            ScopeRef::Agent { agent_id } => ScopeView::Agent { agent_id },
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ServiceInstanceKey {
    pub service_name: String,
    pub scope: ScopeRef,
}

impl ServiceInstanceKey {
    pub fn new(service_name: impl Into<String>, scope: ScopeRef) -> Self {
        Self {
            service_name: service_name.into(),
            scope,
        }
    }

    pub fn instance_id(&self) -> InstanceId {
        InstanceId::from_key(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(Uuid);

impl InstanceId {
    pub fn from_key(key: &ServiceInstanceKey) -> Self {
        let (scope_kind, scope_id) = key.scope.canonical_parts();
        let mut canonical = Vec::new();
        append_part(&mut canonical, &key.service_name);
        append_part(&mut canonical, scope_kind);
        append_part(&mut canonical, scope_id);
        Self(Uuid::new_v5(&INSTANCE_NAMESPACE, &canonical))
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for InstanceId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

fn append_part(target: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let length = u32::try_from(bytes.len()).expect("identity component exceeds u32 length");
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_ids_are_stable_golden_values() {
        let cases = [
            (
                ServiceInstanceKey::new("gitodo", ScopeRef::Store),
                "c81af510-755b-55c7-8487-5668ab36e06e",
            ),
            (
                ServiceInstanceKey::new(
                    "gitodo",
                    ScopeRef::Agent {
                        agent_id: "agent1".to_string(),
                    },
                ),
                "127ce370-1ed6-5b00-9713-e88d01b3010d",
            ),
            (
                ServiceInstanceKey::new(
                    "gitodo",
                    ScopeRef::Agent {
                        agent_id: "agent2".to_string(),
                    },
                ),
                "fbf44981-61a2-5a56-b4c9-f9b20d769665",
            ),
        ];

        for (key, expected) in cases {
            assert_eq!(key.instance_id().to_string(), expected);
        }
    }

    #[test]
    fn identity_does_not_treat_name_characters_as_structure() {
        let key = ServiceInstanceKey::new(
            "a_byagent_b",
            ScopeRef::Agent {
                agent_id: "x:y/z".to_string(),
            },
        );

        assert_eq!(
            key.instance_id().to_string(),
            "a1a8c538-db8d-5185-912d-9480d2652592"
        );
    }

    #[test]
    fn different_service_names_have_distinct_instance_ids() {
        let alpha = ServiceInstanceKey::new("alpha", ScopeRef::Store).instance_id();
        let beta = ServiceInstanceKey::new("beta", ScopeRef::Store).instance_id();

        assert_ne!(alpha, beta);
    }

    #[test]
    fn store_and_agent_named_store_are_distinct() {
        let store = ServiceInstanceKey::new("gitodo", ScopeRef::Store).instance_id();
        let agent = ServiceInstanceKey::new(
            "gitodo",
            ScopeRef::Agent {
                agent_id: "store".to_string(),
            },
        )
        .instance_id();

        assert_ne!(store, agent);
    }

    #[test]
    fn scope_serialization_is_structured() {
        assert_eq!(
            serde_json::to_value(ScopeRef::Store).unwrap(),
            serde_json::json!({ "type": "store" })
        );
        assert_eq!(
            serde_json::to_value(ScopeRef::Agent {
                agent_id: "agent1".to_string(),
            })
            .unwrap(),
            serde_json::json!({ "type": "agent", "agent_id": "agent1" })
        );
    }

    #[test]
    fn scope_view_serialization_is_structured() {
        assert_eq!(
            serde_json::to_value(ScopeView::Root).unwrap(),
            serde_json::json!({ "type": "root" })
        );
        assert_eq!(
            serde_json::to_value(ScopeView::Store).unwrap(),
            serde_json::json!({ "type": "store" })
        );
        assert_eq!(
            serde_json::to_value(ScopeView::Agent {
                agent_id: "agent1".to_string(),
            })
            .unwrap(),
            serde_json::json!({ "type": "agent", "agent_id": "agent1" })
        );
    }

    #[test]
    fn scope_view_round_trips_with_scope_ref() {
        assert_eq!(ScopeView::Root.as_scope_ref(), None);
        assert_eq!(ScopeView::Store.as_scope_ref(), Some(ScopeRef::Store));
        assert_eq!(
            ScopeView::Agent {
                agent_id: "a".to_string(),
            }
            .as_scope_ref(),
            Some(ScopeRef::Agent {
                agent_id: "a".to_string(),
            })
        );
        assert_eq!(ScopeView::from(ScopeRef::Store), ScopeView::Store);
        assert_eq!(
            ScopeView::from(ScopeRef::Agent {
                agent_id: "a".to_string(),
            }),
            ScopeView::Agent {
                agent_id: "a".to_string(),
            }
        );
    }
}
