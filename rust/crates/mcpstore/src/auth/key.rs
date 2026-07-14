use serde::Serialize;

use crate::identity::InstanceId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthCredentialKey {
    instance_id: InstanceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audience: Option<String>,
    client_id: String,
    scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_profile: Option<String>,
}

impl AuthCredentialKey {
    pub fn new(
        instance_id: InstanceId,
        resource: Option<String>,
        audience: Option<String>,
        client_id: impl Into<String>,
        scopes: impl IntoIterator<Item = String>,
        credential_profile: Option<String>,
    ) -> Self {
        let mut scopes = scopes.into_iter().collect::<Vec<_>>();
        scopes.sort();
        scopes.dedup();
        Self {
            instance_id,
            resource,
            audience,
            client_id: client_id.into(),
            scopes,
            credential_profile,
        }
    }

    pub fn scope_hash(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        for scope in &self.scopes {
            hasher.update(&(scope.len() as u64).to_be_bytes());
            hasher.update(scope.as_bytes());
        }
        hasher.finalize().to_hex().to_string()
    }

    pub(crate) fn storage_id(&self) -> String {
        let canonical =
            serde_json::to_vec(self).expect("AuthCredentialKey serialization must always succeed");
        blake3::hash(&canonical).to_hex().to_string()
    }
}
