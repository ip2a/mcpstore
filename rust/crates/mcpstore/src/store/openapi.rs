use crate::openapi::{analyze_openapi_spec, OpenApiImportOptions, OpenApiImportResult};
use crate::openapi_runtime::openapi_tool_infos;
use crate::store::prelude::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct OpenApiImportInput {
    pub name: String,
    pub spec_url: String,
    pub spec: serde_json::Value,
}

pub enum OpenApiImportSource {
    Url,
    ProvidedSpec(serde_json::Value),
}

impl MCPStore {
    pub async fn import_openapi_service(
        &self,
        name: &str,
        spec_url: &str,
    ) -> Result<OpenApiImportResult> {
        self.import_openapi_service_with_options(name, spec_url, OpenApiImportOptions::default())
            .await
    }

    pub async fn import_openapi_service_with_options(
        &self,
        name: &str,
        spec_url: &str,
        options: OpenApiImportOptions,
    ) -> Result<OpenApiImportResult> {
        let spec = reqwest::Client::new()
            .get(spec_url)
            .send()
            .await
            .map_err(|err| StoreError::Other(format!("OpenAPI spec fetch failed: {err}")))?
            .error_for_status()
            .map_err(|err| StoreError::Other(format!("OpenAPI spec fetch failed: {err}")))?
            .json::<serde_json::Value>()
            .await
            .map_err(|err| StoreError::Other(format!("OpenAPI spec JSON decode failed: {err}")))?;
        self.import_openapi_service_from_spec_with_options(name, spec_url, spec, options)
            .await
    }

    pub async fn import_openapi_service_from_spec(
        &self,
        name: &str,
        spec_url: &str,
        spec: serde_json::Value,
    ) -> Result<OpenApiImportResult> {
        self.import_openapi_service_from_spec_with_options(
            name,
            spec_url,
            spec,
            OpenApiImportOptions::default(),
        )
        .await
    }

    pub async fn import_openapi_service_from_spec_with_options(
        &self,
        name: &str,
        spec_url: &str,
        spec: serde_json::Value,
        options: OpenApiImportOptions,
    ) -> Result<OpenApiImportResult> {
        let mut result = analyze_openapi_spec(name, spec_url, spec)?;
        result.runtime_executable = true;
        self.register_openapi_virtual_service(&result, &options)
            .await?;
        let now = chrono::Utc::now().timestamp();
        let value = serde_json::to_value(&result).map_err(|err| {
            StoreError::Other(format!("OpenAPI import result serialization failed: {err}"))
        })?;
        self.cache.put_state("openapi_imports", name, value).await?;
        self.cache
            .put_event(
                "openapi_imports",
                &format!("{name}:imported:{now}"),
                serde_json::json!({
                    "event": "openapi_imported",
                    "service_name": name,
                    "spec_url": spec_url,
                    "timestamp": now,
                    "total_endpoints": result.total_endpoints,
                    "runtime_executable": result.runtime_executable,
                }),
            )
            .await?;
        Ok(result)
    }

    pub(crate) async fn register_openapi_virtual_service(
        &self,
        result: &OpenApiImportResult,
        options: &OpenApiImportOptions,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let config = ServerConfig {
            url: if result.base_url.is_empty() {
                None
            } else {
                Some(result.base_url.clone())
            },
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            headers: options.headers.clone(),
            transport: Some("openapi".to_string()),
            working_dir: None,
            description: result.spec_info.description.clone(),
        };
        let config_value = openapi_config_value(&config, options)?;
        let tools = openapi_tool_infos(result);
        let entry = ServiceEntry {
            name: result.service_name.clone(),
            original_name: result.service_name.clone(),
            agent_id: GLOBAL_AGENT_STORE.to_string(),
            transport: "openapi".to_string(),
            url: config.url.clone(),
            command: None,
            status: ConnectionStatus::Connected,
            tools: tools.clone(),
            config: config_value.clone(),
            added_time: now,
        };
        self.registry.register(entry).await;

        let entity = ServiceEntity {
            service_global_name: result.service_name.clone(),
            service_original_name: result.service_name.clone(),
            source_agent: GLOBAL_AGENT_STORE.to_string(),
            config: config_value,
            added_time: now,
        };
        self.cache
            .put_entity(
                "services",
                &result.service_name,
                serde_json::to_value(entity).unwrap_or_default(),
            )
            .await?;
        self.upsert_agent_service_relation(GLOBAL_AGENT_STORE, &result.service_name, now)
            .await?;

        let mut relation_tools = Vec::with_capacity(tools.len());
        let mut status_tools = Vec::with_capacity(tools.len());
        for tool in &tools {
            let global_name = generate_tool_global_name(&result.service_name, &tool.name)?;
            let entity = ToolEntity {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
                service_global_name: result.service_name.clone(),
                service_original_name: result.service_name.clone(),
                source_agent: GLOBAL_AGENT_STORE.to_string(),
                description: tool.description.clone(),
                input_schema: tool.schema.clone(),
                created_time: now,
                tool_hash: openapi_tool_content_hash(&result.service_name, tool),
            };
            self.cache
                .put_entity(
                    "tools",
                    &global_name,
                    serde_json::to_value(entity).unwrap_or_default(),
                )
                .await?;
            relation_tools.push(ToolRelationItem {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
            });
            status_tools.push(ToolStatusItem {
                tool_global_name: global_name,
                tool_original_name: tool.name.clone(),
                status: ToolAvailability::Available,
            });
        }

        self.cache
            .put_relation(
                "service_tools",
                &result.service_name,
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: result.service_name.clone(),
                    service_original_name: result.service_name.clone(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    tools: relation_tools,
                })
                .unwrap_or_default(),
            )
            .await?;
        let status = self.service_status_payload(
            &result.service_name,
            HealthStatus::Healthy,
            None,
            status_tools,
        );
        self.cache
            .put_state(
                "service_status",
                &result.service_name,
                serde_json::to_value(status).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    pub async fn get_openapi_import(&self, name: &str) -> Result<Option<OpenApiImportResult>> {
        let Some(value) = self.cache.get_state("openapi_imports", name).await? else {
            return Ok(None);
        };
        serde_json::from_value(value).map(Some).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI import result deserialization failed: {err}"
            ))
        })
    }

    pub async fn list_openapi_imports(&self) -> Result<Vec<OpenApiImportResult>> {
        let values = self.cache.get_all_states_async("openapi_imports").await?;
        let mut imports: Vec<OpenApiImportResult> = Vec::with_capacity(values.len());
        for value in values.into_values() {
            imports.push(serde_json::from_value(value).map_err(|err| {
                StoreError::Other(format!(
                    "OpenAPI import result deserialization failed: {err}"
                ))
            })?);
        }
        imports.sort_by(|left, right| left.service_name.cmp(&right.service_name));
        Ok(imports)
    }
}

impl MCPStore {
    pub(crate) async fn openapi_runtime_options(
        &self,
        service_name: &str,
    ) -> Result<OpenApiImportOptions> {
        let Some(service) = self.registry.find_service(service_name).await else {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        };
        let config_value = service.config.clone();
        let config: ServerConfig = serde_json::from_value(config_value.clone()).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI service config deserialization failed: {err}"
            ))
        })?;
        let auth = config_value
            .get("openapi_auth")
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        Ok(OpenApiImportOptions {
            headers: config.headers,
            auth,
        })
    }
}

fn openapi_config_value(
    config: &ServerConfig,
    options: &OpenApiImportOptions,
) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(config).map_err(|err| {
        StoreError::Other(format!(
            "OpenAPI service config serialization failed: {err}"
        ))
    })?;
    if !options.auth.is_empty() {
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "openapi_auth".to_string(),
                serde_json::Value::Object(options.auth.clone()),
            );
        }
    }
    Ok(value)
}

fn openapi_tool_content_hash(name: &str, tool: &crate::registry::ToolInfo) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    tool.name.hash(&mut hasher);
    tool.description.hash(&mut hasher);
    serde_json::to_string(&tool.schema)
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
