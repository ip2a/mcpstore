use crate::openapi::{
    analyze_openapi_spec, parse_openapi_spec_text, resolve_openapi_local_refs,
    OpenApiImportOptions, OpenApiImportResult,
};
use crate::openapi_runtime::openapi_tool_infos;
use crate::store::prelude::*;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;

const MAX_EXTERNAL_REF_DEPTH: usize = 32;

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
        let client = reqwest::Client::new();
        let spec_text = fetch_openapi_spec_text(&client, spec_url).await?;
        self.import_openapi_service_from_spec_text_with_options(name, spec_url, &spec_text, options)
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

    pub async fn import_openapi_service_from_spec_text(
        &self,
        name: &str,
        spec_url: &str,
        spec_text: &str,
    ) -> Result<OpenApiImportResult> {
        self.import_openapi_service_from_spec_text_with_options(
            name,
            spec_url,
            spec_text,
            OpenApiImportOptions::default(),
        )
        .await
    }

    pub async fn import_openapi_service_from_spec_text_with_options(
        &self,
        name: &str,
        spec_url: &str,
        spec_text: &str,
        options: OpenApiImportOptions,
    ) -> Result<OpenApiImportResult> {
        let spec = parse_openapi_spec_text(spec_text)?;
        self.import_openapi_service_from_spec_with_options(name, spec_url, spec, options)
            .await
    }

    pub async fn import_openapi_service_from_spec_with_options(
        &self,
        name: &str,
        spec_url: &str,
        spec: serde_json::Value,
        options: OpenApiImportOptions,
    ) -> Result<OpenApiImportResult> {
        let client = reqwest::Client::new();
        let spec = bundle_openapi_external_refs(&client, spec_url, spec).await?;
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

    pub async fn bundle_openapi_spec(&self, spec_url: &str) -> Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let spec_text = fetch_openapi_spec_text(&client, spec_url).await?;
        self.bundle_openapi_spec_from_text(spec_url, &spec_text)
            .await
    }

    pub async fn bundle_openapi_spec_from_text(
        &self,
        spec_url: &str,
        spec_text: &str,
    ) -> Result<serde_json::Value> {
        let spec = parse_openapi_spec_text(spec_text)?;
        self.bundle_openapi_spec_from_value(spec_url, spec).await
    }

    pub async fn bundle_openapi_spec_from_value(
        &self,
        spec_url: &str,
        spec: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let client = reqwest::Client::new();
        bundle_openapi_external_refs(&client, spec_url, spec).await
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

async fn bundle_openapi_external_refs(
    client: &reqwest::Client,
    document_url: &str,
    spec: serde_json::Value,
) -> Result<serde_json::Value> {
    let resolver = OpenApiExternalRefResolver::new(client);
    resolver
        .resolve_external_refs(document_url.to_string(), spec, 0, Vec::new())
        .await
}

async fn fetch_openapi_spec_text(client: &reqwest::Client, spec_url: &str) -> Result<String> {
    if is_http_url(spec_url) {
        return client
            .get(spec_url)
            .send()
            .await
            .map_err(|err| StoreError::Other(format!("OpenAPI spec fetch failed: {err}")))?
            .error_for_status()
            .map_err(|err| StoreError::Other(format!("OpenAPI spec fetch failed: {err}")))?
            .text()
            .await
            .map_err(|err| StoreError::Other(format!("OpenAPI spec body read failed: {err}")));
    }

    if spec_url.starts_with("file://") || reqwest::Url::parse(spec_url).is_err() {
        let path = path_from_file_target(spec_url)?;
        return read_openapi_document_file(&path, spec_url);
    }

    Err(StoreError::Other(format!(
        "Unsupported OpenAPI spec URL: {spec_url}"
    )))
}

struct OpenApiExternalRefResolver<'a> {
    client: &'a reqwest::Client,
    documents: Mutex<HashMap<String, serde_json::Value>>,
}

impl<'a> OpenApiExternalRefResolver<'a> {
    fn new(client: &'a reqwest::Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_external_refs(
        &'a self,
        document_url: String,
        value: serde_json::Value,
        depth: usize,
        ref_stack: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send + 'a>> {
        Box::pin(async move {
            if depth > MAX_EXTERNAL_REF_DEPTH {
                return Err(StoreError::Other(
                    "OpenAPI external $ref resolution exceeded maximum depth".to_string(),
                ));
            }

            match value {
                serde_json::Value::Object(map) => {
                    if let Some(reference) = map
                        .get("$ref")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                    {
                        if !reference.starts_with('#') {
                            let mut resolved = self
                                .fetch_external_ref(
                                    &document_url,
                                    &reference,
                                    depth + 1,
                                    ref_stack.clone(),
                                )
                                .await?;
                            if let serde_json::Value::Object(resolved_map) = &mut resolved {
                                for (key, sibling) in
                                    map.into_iter().filter(|(key, _)| key != "$ref")
                                {
                                    resolved_map.insert(
                                        key,
                                        self.resolve_external_refs(
                                            document_url.clone(),
                                            sibling,
                                            depth + 1,
                                            ref_stack.clone(),
                                        )
                                        .await?,
                                    );
                                }
                            }
                            return Ok(resolved);
                        }
                    }

                    let mut resolved = serde_json::Map::new();
                    for (key, child) in map {
                        resolved.insert(
                            key,
                            self.resolve_external_refs(
                                document_url.clone(),
                                child,
                                depth + 1,
                                ref_stack.clone(),
                            )
                            .await?,
                        );
                    }
                    Ok(serde_json::Value::Object(resolved))
                }
                serde_json::Value::Array(items) => {
                    let mut resolved = Vec::with_capacity(items.len());
                    for item in items {
                        resolved.push(
                            self.resolve_external_refs(
                                document_url.clone(),
                                item,
                                depth + 1,
                                ref_stack.clone(),
                            )
                            .await?,
                        );
                    }
                    Ok(serde_json::Value::Array(resolved))
                }
                other => Ok(other),
            }
        })
    }

    async fn fetch_external_ref(
        &self,
        document_url: &str,
        reference: &str,
        depth: usize,
        ref_stack: Vec<String>,
    ) -> Result<serde_json::Value> {
        let (target_url, pointer) = split_external_ref(document_url, reference)?;
        let ref_key = match &pointer {
            Some(pointer) => format!("{target_url}#{pointer}"),
            None => target_url.clone(),
        };
        if ref_stack.iter().any(|item| item == &ref_key) {
            let mut cycle = ref_stack;
            cycle.push(ref_key);
            return Err(StoreError::Other(format!(
                "OpenAPI external $ref cycle detected: {}",
                cycle.join(" -> ")
            )));
        }

        let mut next_stack = ref_stack;
        next_stack.push(ref_key);
        let document = self.fetch_external_document(&target_url).await?;
        let bundled = self
            .resolve_external_refs(target_url.clone(), document, depth, next_stack)
            .await?;
        let target = if let Some(pointer) = pointer {
            bundled.pointer(&pointer).cloned().ok_or_else(|| {
                StoreError::Other(format!(
                    "OpenAPI external $ref target not found: {reference}"
                ))
            })?
        } else {
            bundled.clone()
        };
        resolve_openapi_local_refs(&bundled, &target)
    }

    async fn fetch_external_document(&self, target_url: &str) -> Result<serde_json::Value> {
        if let Some(document) = self
            .documents
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI external $ref cache lock poisoned".to_string())
            })?
            .get(target_url)
        {
            return Ok(document.clone());
        }

        let document_text = if is_http_url(target_url) {
            self.client
                .get(target_url)
                .send()
                .await
                .map_err(|err| {
                    StoreError::Other(format!(
                        "OpenAPI external $ref fetch failed for {target_url}: {err}"
                    ))
                })?
                .error_for_status()
                .map_err(|err| {
                    StoreError::Other(format!(
                        "OpenAPI external $ref fetch failed for {target_url}: {err}"
                    ))
                })?
                .text()
                .await
                .map_err(|err| {
                    StoreError::Other(format!(
                        "OpenAPI external $ref body read failed for {target_url}: {err}"
                    ))
                })?
        } else if target_url.starts_with("file://") {
            let path = path_from_file_target(target_url)?;
            read_openapi_document_file(&path, target_url)?
        } else {
            return Err(StoreError::Other(format!(
                "Unsupported OpenAPI external $ref URL: {target_url}"
            )));
        };
        let document = parse_openapi_spec_text(&document_text).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI external $ref document decode failed for {target_url}: {err}"
            ))
        })?;
        self.documents
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI external $ref cache lock poisoned".to_string())
            })?
            .insert(target_url.to_string(), document.clone());
        Ok(document)
    }
}

fn split_external_ref(document_url: &str, reference: &str) -> Result<(String, Option<String>)> {
    let (target, fragment) = reference.split_once('#').unwrap_or((reference, ""));
    let target_url = if target.is_empty() {
        normalize_document_target(document_url)?
    } else if is_http_url(target) {
        target.to_string()
    } else if target.starts_with("file://") || Path::new(target).is_absolute() {
        normalize_file_document_target(target)?
    } else if reqwest::Url::parse(target).is_ok() {
        return Err(StoreError::Other(format!(
            "Unsupported OpenAPI external $ref URL: {reference}"
        )));
    } else {
        join_external_ref_target(document_url, target, reference)?
    };
    let pointer = if fragment.is_empty() {
        None
    } else if fragment.starts_with('/') {
        Some(fragment.to_string())
    } else {
        return Err(StoreError::Other(format!(
            "Invalid OpenAPI external $ref fragment: {reference}"
        )));
    };
    Ok((target_url, pointer))
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn normalize_document_target(document_url: &str) -> Result<String> {
    if is_http_url(document_url) {
        Ok(document_url.to_string())
    } else if document_url.starts_with("file://") || reqwest::Url::parse(document_url).is_err() {
        normalize_file_document_target(document_url)
    } else {
        Err(StoreError::Other(format!(
            "Unsupported OpenAPI external $ref URL: {document_url}"
        )))
    }
}

fn join_external_ref_target(document_url: &str, target: &str, reference: &str) -> Result<String> {
    if let Ok(base) = reqwest::Url::parse(document_url) {
        match base.scheme() {
            "http" | "https" => base.join(target).map(|url| url.to_string()).map_err(|err| {
                StoreError::Other(format!("Invalid OpenAPI external $ref {reference}: {err}"))
            }),
            "file" => {
                let joined = base.join(target).map_err(|err| {
                    StoreError::Other(format!("Invalid OpenAPI external $ref {reference}: {err}"))
                })?;
                normalize_file_document_target(joined.as_str())
            }
            _ => Err(StoreError::Other(format!(
                "OpenAPI relative external $ref requires an absolute HTTP(S) or file spec_url: {reference}"
            ))),
        }
    } else {
        let base_path = absolute_path(Path::new(document_url))?;
        let base_dir = base_path.parent().ok_or_else(|| {
            StoreError::Other(format!(
                "OpenAPI relative external $ref has no parent document path: {reference}"
            ))
        })?;
        file_url_from_path(&base_dir.join(target))
    }
}

fn normalize_file_document_target(target: &str) -> Result<String> {
    let path = path_from_file_target(target)?;
    file_url_from_path(&path)
}

fn path_from_file_target(target: &str) -> Result<PathBuf> {
    if target.starts_with("file://") {
        let url = reqwest::Url::parse(target).map_err(|err| {
            StoreError::Other(format!("Invalid OpenAPI file URL {target}: {err}"))
        })?;
        if url.scheme() != "file" {
            return Err(StoreError::Other(format!(
                "Unsupported OpenAPI file URL: {target}"
            )));
        }
        url.to_file_path()
            .map_err(|_| StoreError::Other(format!("Invalid OpenAPI file URL path: {target}")))
    } else {
        absolute_path(Path::new(target))
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current_dir| current_dir.join(path))
            .map_err(|err| StoreError::Other(format!("OpenAPI current dir read failed: {err}")))
    }
}

fn file_url_from_path(path: &Path) -> Result<String> {
    let absolute = absolute_path(path)?;
    reqwest::Url::from_file_path(&absolute)
        .map(|url| url.to_string())
        .map_err(|_| {
            StoreError::Other(format!("Invalid OpenAPI file path: {}", absolute.display()))
        })
}

fn read_openapi_document_file(path: &Path, label: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|err| StoreError::Other(format!("OpenAPI file read failed for {label}: {err}")))
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
