use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::config::ServerConfig;
use crate::transport::client::McpConnection;
use crate::transport::{Result, ToolCallResult, ToolDescription, TransportError};

pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<String, McpConnection>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add(&self, name: String, config: ServerConfig) {
        let conn = McpConnection::new(name.clone(), config);
        self.connections.write().await.insert(name, conn);
    }

    pub async fn connect(&self, name: &str) -> Result<()> {
        let mut conns = self.connections.write().await;
        let conn = conns
            .get_mut(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        if conn.is_connected() {
            return Ok(());
        }
        conn.connect().await
    }

    pub async fn disconnect(&self, name: &str) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(conn) = conns.get_mut(name) {
            conn.disconnect().await?;
        }
        Ok(())
    }

    pub async fn remove(&self, name: &str) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(mut conn) = conns.remove(name) {
            conn.disconnect().await.ok();
        }
        Ok(())
    }

    pub async fn clear(&self) {
        let names: Vec<String> = self.connections.read().await.keys().cloned().collect();
        for name in names {
            self.remove(&name).await.ok();
        }
    }

    pub async fn list_tools(&self, name: &str) -> Result<Vec<ToolDescription>> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.list_tools().await
    }

    pub async fn call_tool(
        &self,
        name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.call_tool(tool_name, args).await
    }

    pub async fn list_resources(&self, name: &str) -> Result<Vec<serde_json::Value>> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.list_resources().await
    }

    pub async fn list_resource_templates(&self, name: &str) -> Result<Vec<serde_json::Value>> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.list_resource_templates().await
    }

    pub async fn read_resource(&self, name: &str, uri: &str) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.read_resource(uri).await
    }

    pub async fn list_prompts(&self, name: &str) -> Result<Vec<serde_json::Value>> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.list_prompts().await
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(name)
            .ok_or_else(|| TransportError::NotConnected(format!("Service not found: {name}")))?;
        conn.get_prompt(prompt_name, arguments).await
    }

    pub async fn is_connected(&self, name: &str) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(name)
            .map(McpConnection::is_connected)
            .unwrap_or(false)
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
