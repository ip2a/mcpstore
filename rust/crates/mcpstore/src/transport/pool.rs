use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::auth::AuthCoordinator;
use crate::config::ServerConfig;
use crate::identity::InstanceId;
use crate::transport::client::McpConnection;
use crate::transport::{
    DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate, DiscoveredTool, Result,
    ToolCallResult, TransportError,
};

pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<InstanceId, McpConnection>>>,
    auth_coordinator: AuthCoordinator,
}

impl ConnectionPool {
    pub fn new(auth_coordinator: AuthCoordinator) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            auth_coordinator,
        }
    }

    pub async fn add(&self, instance_id: InstanceId, config: ServerConfig) {
        let conn = McpConnection::new(
            instance_id,
            instance_id.to_string(),
            config,
            self.auth_coordinator.clone(),
        );
        self.connections.write().await.insert(instance_id, conn);
    }

    pub async fn connect(&self, instance_id: InstanceId) -> Result<()> {
        let mut conns = self.connections.write().await;
        let conn = conns.get_mut(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        if conn.is_connected() {
            return Ok(());
        }
        conn.connect().await
    }

    pub async fn disconnect(&self, instance_id: InstanceId) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(conn) = conns.get_mut(&instance_id) {
            conn.disconnect().await?;
        }
        Ok(())
    }

    pub async fn remove(&self, instance_id: InstanceId) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(mut conn) = conns.remove(&instance_id) {
            conn.disconnect().await.ok();
        }
        Ok(())
    }

    pub async fn clear(&self) {
        let instance_ids: Vec<InstanceId> = self.connections.read().await.keys().copied().collect();
        for instance_id in instance_ids {
            self.remove(instance_id).await.ok();
        }
    }

    pub async fn list_tools(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredTool>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_tools().await
    }

    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.call_tool(tool_name, args).await
    }

    pub async fn list_resources(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredResource>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_resources().await
    }

    pub async fn list_resource_templates(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<DiscoveredResourceTemplate>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_resource_templates().await
    }

    pub async fn read_resource(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.read_resource(uri).await
    }

    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_prompts().await
    }

    pub async fn get_prompt(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.get_prompt(prompt_name, arguments).await
    }

    pub async fn is_connected(&self, instance_id: InstanceId) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(&instance_id)
            .map(McpConnection::is_connected)
            .unwrap_or(false)
    }
}
