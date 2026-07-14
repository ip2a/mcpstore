use crate::config::ScopeDeclarations;
use crate::openapi::{
    analyze_openapi_spec, parse_openapi_spec_text, resolve_openapi_local_refs,
    OpenApiBundleArtifact, OpenApiBundleDependency, OpenApiBundleDiagnostic, OpenApiBundleDocument,
    OpenApiBundleOptions, OpenApiImportOptions, OpenApiImportResult, OpenApiRefCachePolicy,
};
use crate::store::prelude::*;
use crate::store::CacheLayerManager;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

const MAX_EXTERNAL_REF_DEPTH: usize = 32;
const OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE: &str = "openapi_ref_documents";
const OPENAPI_REF_DOCUMENT_CACHE_VERSION: u64 = 1;
const OPENAPI_IMPORT_CONTEXT_STATE_TYPE: &str = "openapi_import_context";
const OPENAPI_IMPORT_CONTEXT_KEY: &str = "global";
const OPENAPI_IMPORT_CONTEXT_VERSION: u64 = 1;

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
        let client = openapi_http_client(options.fetch_timeout_millis)?;
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
        if self.registry.find_definition(name).await.is_some() {
            return Err(StoreError::Other(format!(
                "Service definition already exists: {name}"
            )));
        }

        let client = openapi_http_client(options.fetch_timeout_millis)?;
        let bundle_options = OpenApiBundleOptions {
            ref_cache: options.ref_cache.clone(),
            timeout_millis: options.fetch_timeout_millis,
        };
        let spec =
            bundle_openapi_external_refs(&client, &self.cache, spec_url, spec, &bundle_options)
                .await?;
        let mut result = analyze_openapi_spec(name, spec_url, spec)?;
        result.runtime_executable = true;
        self.register_openapi_virtual_service(&result, &options)
            .await?;
        let now = chrono::Utc::now().timestamp();
        let value = serde_json::to_value(&result).map_err(|err| {
            StoreError::Other(format!("OpenAPI import result serialization failed: {err}"))
        })?;
        self.cache.put_state("openapi_imports", name, value).await?;
        let context = OpenApiImportContextState {
            last_service_name: name.to_string(),
            updated_at: now,
            version: OPENAPI_IMPORT_CONTEXT_VERSION,
        };
        self.cache
            .put_state(
                OPENAPI_IMPORT_CONTEXT_STATE_TYPE,
                OPENAPI_IMPORT_CONTEXT_KEY,
                serde_json::to_value(context).map_err(|err| {
                    StoreError::Other(format!(
                        "OpenAPI import context serialization failed: {err}"
                    ))
                })?,
            )
            .await?;
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
        self.bundle_openapi_spec_with_options(spec_url, OpenApiBundleOptions::default())
            .await
    }

    pub async fn bundle_openapi_spec_with_options(
        &self,
        spec_url: &str,
        options: OpenApiBundleOptions,
    ) -> Result<serde_json::Value> {
        let client = openapi_http_client(options.timeout_millis)?;
        let spec_text = fetch_openapi_spec_text(&client, spec_url).await?;
        self.bundle_openapi_spec_from_text_with_options(spec_url, &spec_text, options)
            .await
    }

    pub async fn bundle_openapi_spec_from_text(
        &self,
        spec_url: &str,
        spec_text: &str,
    ) -> Result<serde_json::Value> {
        self.bundle_openapi_spec_from_text_with_options(
            spec_url,
            spec_text,
            OpenApiBundleOptions::default(),
        )
        .await
    }

    pub async fn bundle_openapi_spec_from_text_with_options(
        &self,
        spec_url: &str,
        spec_text: &str,
        options: OpenApiBundleOptions,
    ) -> Result<serde_json::Value> {
        let spec = parse_openapi_spec_text(spec_text)?;
        self.bundle_openapi_spec_from_value_with_options(spec_url, spec, options)
            .await
    }

    pub async fn bundle_openapi_spec_from_value(
        &self,
        spec_url: &str,
        spec: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.bundle_openapi_spec_from_value_with_options(
            spec_url,
            spec,
            OpenApiBundleOptions::default(),
        )
        .await
    }

    pub async fn bundle_openapi_spec_from_value_with_options(
        &self,
        spec_url: &str,
        spec: serde_json::Value,
        options: OpenApiBundleOptions,
    ) -> Result<serde_json::Value> {
        let client = openapi_http_client(options.timeout_millis)?;
        bundle_openapi_external_refs(&client, &self.cache, spec_url, spec, &options).await
    }

    pub async fn bundle_openapi_artifact(&self, spec_url: &str) -> Result<OpenApiBundleArtifact> {
        self.bundle_openapi_artifact_with_options(spec_url, OpenApiBundleOptions::default())
            .await
    }

    pub async fn bundle_openapi_artifact_with_options(
        &self,
        spec_url: &str,
        options: OpenApiBundleOptions,
    ) -> Result<OpenApiBundleArtifact> {
        let client = openapi_http_client(options.timeout_millis)?;
        let spec_text = fetch_openapi_spec_text(&client, spec_url).await?;
        self.bundle_openapi_artifact_from_text_with_options(spec_url, &spec_text, options)
            .await
    }

    pub async fn bundle_openapi_artifact_from_text(
        &self,
        spec_url: &str,
        spec_text: &str,
    ) -> Result<OpenApiBundleArtifact> {
        self.bundle_openapi_artifact_from_text_with_options(
            spec_url,
            spec_text,
            OpenApiBundleOptions::default(),
        )
        .await
    }

    pub async fn bundle_openapi_artifact_from_text_with_options(
        &self,
        spec_url: &str,
        spec_text: &str,
        options: OpenApiBundleOptions,
    ) -> Result<OpenApiBundleArtifact> {
        let spec = parse_openapi_spec_text(spec_text)?;
        let client = openapi_http_client(options.timeout_millis)?;
        let root_metadata = document_metadata_from_bytes(spec_text.as_bytes());
        bundle_openapi_external_ref_artifact(
            &client,
            &self.cache,
            spec_url,
            spec,
            root_metadata,
            &options,
        )
        .await
    }

    pub async fn bundle_openapi_artifact_from_value(
        &self,
        spec_url: &str,
        spec: serde_json::Value,
    ) -> Result<OpenApiBundleArtifact> {
        self.bundle_openapi_artifact_from_value_with_options(
            spec_url,
            spec,
            OpenApiBundleOptions::default(),
        )
        .await
    }

    pub async fn bundle_openapi_artifact_from_value_with_options(
        &self,
        spec_url: &str,
        spec: serde_json::Value,
        options: OpenApiBundleOptions,
    ) -> Result<OpenApiBundleArtifact> {
        let client = openapi_http_client(options.timeout_millis)?;
        let root_metadata = document_metadata_from_value(&spec)?;
        bundle_openapi_external_ref_artifact(
            &client,
            &self.cache,
            spec_url,
            spec,
            root_metadata,
            &options,
        )
        .await
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
            auth: Default::default(),
            transport: Some("openapi".to_string()),
            working_dir: None,
            description: result.spec_info.description.clone(),
            mcpstore: None,
            extra: serde_json::Map::new(),
        };
        let config_value = openapi_config_value(&config, options)?;
        let base_config = config_value.as_object().cloned().ok_or_else(|| {
            StoreError::Other("OpenAPI service config must serialize as an object".to_string())
        })?;
        let scopes = ScopeDeclarations::store_only();
        let definition = ServiceDefinition {
            service_name: result.service_name.clone(),
            base_config: base_config.clone(),
            scopes: scopes.clone(),
            lifecycle: None,
            base_revision: 1,
            metadata: serde_json::Map::new(),
            added_time: now,
        };
        let instance_id =
            ServiceInstanceKey::new(&result.service_name, ScopeRef::Store).instance_id();
        let revision = ConfigRevision {
            base_revision: 1,
            scope_revision: 1,
        };
        let instance = ServiceInstance {
            instance_id,
            service_name: result.service_name.clone(),
            scope: ScopeRef::Store,
            transport: "openapi".to_string(),
            url: config.url.clone(),
            command: None,
            status: ConnectionStatus::Disconnected,
            tools: Vec::new(),
            effective_config: base_config.clone(),
            config_revision: revision,
            applied_config_revision: None,
            added_time: now,
        };
        self.registry.register_definition(definition).await;
        self.registry.register_instance(instance).await;
        self.cache_instance_added(instance_id).await?;
        self.set_instance_status(instance_id, HealthStatus::Disconnected, None, Vec::new())
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

    pub async fn last_openapi_import(&self) -> Result<Option<OpenApiImportResult>> {
        let Some(value) = self
            .cache
            .get_state(
                OPENAPI_IMPORT_CONTEXT_STATE_TYPE,
                OPENAPI_IMPORT_CONTEXT_KEY,
            )
            .await?
        else {
            return Ok(None);
        };
        let context: OpenApiImportContextState = serde_json::from_value(value).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI import context deserialization failed: {err}"
            ))
        })?;
        self.get_openapi_import(&context.last_service_name).await
    }

    pub(crate) async fn clear_openapi_import_for_service(&self, name: &str) -> Result<()> {
        self.cache.delete_state("openapi_imports", name).await?;
        let Some(value) = self
            .cache
            .get_state(
                OPENAPI_IMPORT_CONTEXT_STATE_TYPE,
                OPENAPI_IMPORT_CONTEXT_KEY,
            )
            .await?
        else {
            return Ok(());
        };
        let context: OpenApiImportContextState = serde_json::from_value(value).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI import context deserialization failed: {err}"
            ))
        })?;
        if context.last_service_name == name {
            self.cache
                .delete_state(
                    OPENAPI_IMPORT_CONTEXT_STATE_TYPE,
                    OPENAPI_IMPORT_CONTEXT_KEY,
                )
                .await?;
        }
        Ok(())
    }
}

impl MCPStore {
    pub(crate) async fn openapi_runtime_options_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<OpenApiImportOptions> {
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        let applied_config = self
            .applied_openapi_configs
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "OpenAPI instance {instance_id} has no applied runtime config"
                ))
            })?;
        let config_value = serde_json::Value::Object(applied_config);
        let config: ServerConfig = serde_json::from_value(config_value.clone()).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI instance config deserialization failed for {instance_id}: {err}"
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
            ref_cache: OpenApiRefCachePolicy::default(),
            timeout_millis: config_value
                .get("openapi_timeout_millis")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(crate::openapi::OpenApiImportOptions::default_timeout_millis),
            fetch_timeout_millis: config_value
                .get("openapi_fetch_timeout_millis")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(crate::openapi::OpenApiImportOptions::default_fetch_timeout_millis),
        })
    }
}

fn openapi_http_client(timeout_millis: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_millis.max(1)))
        .build()
        .map_err(|err| StoreError::Other(format!("OpenAPI HTTP client creation failed: {err}")))
}

async fn bundle_openapi_external_refs(
    client: &reqwest::Client,
    cache: &CacheLayerManager,
    document_url: &str,
    spec: serde_json::Value,
    options: &OpenApiBundleOptions,
) -> Result<serde_json::Value> {
    let root_metadata = document_metadata_from_value(&spec)?;
    Ok(bundle_openapi_external_ref_artifact(
        client,
        cache,
        document_url,
        spec,
        root_metadata,
        options,
    )
    .await?
    .bundle)
}

async fn bundle_openapi_external_ref_artifact(
    client: &reqwest::Client,
    cache: &CacheLayerManager,
    document_url: &str,
    spec: serde_json::Value,
    root_metadata: OpenApiBundleDocumentMetadata,
    options: &OpenApiBundleOptions,
) -> Result<OpenApiBundleArtifact> {
    let root_document = document_label(document_url);
    let resolver = OpenApiExternalRefResolver::new(
        client,
        cache,
        root_document.clone(),
        root_metadata,
        options.ref_cache.clone(),
    );
    let bundle = resolver
        .resolve_external_refs(document_url.to_string(), spec, 0, Vec::new())
        .await?;
    Ok(resolver.into_artifact(document_url.to_string(), root_document, bundle)?)
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
    cache: &'a CacheLayerManager,
    ref_cache: OpenApiRefCachePolicy,
    documents: Mutex<HashMap<String, serde_json::Value>>,
    document_metadata: Mutex<HashMap<String, OpenApiBundleDocumentMetadata>>,
    loaded_documents: Mutex<Vec<String>>,
    dependencies: Mutex<Vec<OpenApiBundleDependency>>,
    diagnostics: Mutex<Vec<OpenApiBundleDiagnostic>>,
}

#[derive(Debug, Clone)]
struct OpenApiBundleDocumentMetadata {
    content_hash: String,
    content_length: usize,
}

#[derive(Debug, Clone)]
struct OpenApiFileDocumentFingerprint {
    file_size: u64,
    file_modified_unix_millis: i64,
}

#[derive(Debug, Clone)]
struct OpenApiCachedHttpDocument {
    document: serde_json::Value,
    metadata: OpenApiBundleDocumentMetadata,
    expires_at: i64,
    etag: Option<String>,
    last_modified: Option<String>,
}

struct OpenApiHttpDocumentResponse {
    text: String,
    etag: Option<String>,
    last_modified: Option<String>,
}

enum OpenApiHttpDocumentFetch {
    Modified(OpenApiHttpDocumentResponse),
    NotModified,
}

impl<'a> OpenApiExternalRefResolver<'a> {
    fn new(
        client: &'a reqwest::Client,
        cache: &'a CacheLayerManager,
        root_document: String,
        root_metadata: OpenApiBundleDocumentMetadata,
        ref_cache: OpenApiRefCachePolicy,
    ) -> Self {
        let mut document_metadata = HashMap::new();
        document_metadata.insert(root_document.clone(), root_metadata);
        Self {
            client,
            cache,
            ref_cache,
            documents: Mutex::new(HashMap::new()),
            document_metadata: Mutex::new(document_metadata),
            loaded_documents: Mutex::new(vec![root_document]),
            dependencies: Mutex::new(Vec::new()),
            diagnostics: Mutex::new(Vec::new()),
        }
    }

    fn into_artifact(
        self,
        spec_url: String,
        root_document: String,
        bundle: serde_json::Value,
    ) -> Result<OpenApiBundleArtifact> {
        let mut urls = self.loaded_documents.into_inner().map_err(|_| {
            StoreError::Other("OpenAPI bundle document list lock poisoned".to_string())
        })?;
        urls.sort();
        urls.dedup();
        let document_metadata = self.document_metadata.into_inner().map_err(|_| {
            StoreError::Other("OpenAPI bundle document metadata lock poisoned".to_string())
        })?;
        let documents = urls
            .into_iter()
            .map(|url| {
                let metadata = document_metadata.get(&url).cloned().ok_or_else(|| {
                    StoreError::Other(format!(
                        "OpenAPI bundle document metadata missing for {url}"
                    ))
                })?;
                Ok(OpenApiBundleDocument {
                    role: if url == root_document {
                        "root"
                    } else {
                        "external"
                    }
                    .to_string(),
                    content_hash: metadata.content_hash,
                    content_length: metadata.content_length,
                    url,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let dependencies = self.dependencies.into_inner().map_err(|_| {
            StoreError::Other("OpenAPI bundle dependency list lock poisoned".to_string())
        })?;
        let diagnostics = self.diagnostics.into_inner().map_err(|_| {
            StoreError::Other("OpenAPI bundle diagnostics lock poisoned".to_string())
        })?;
        Ok(OpenApiBundleArtifact {
            spec_url,
            bundle,
            documents,
            dependencies,
            diagnostics,
        })
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
        self.record_dependency(document_url, reference, &target_url, pointer.as_deref())?;
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

        let file_path = if target_url.starts_with("file://") {
            Some(path_from_file_target(target_url)?)
        } else {
            None
        };

        let mut file_metadata = None;
        let mut file_fingerprint = None;
        let mut file_document_text = None;

        let mut http_response = None;

        if is_http_url(target_url) {
            let cached_http_document = if self.ref_cache.is_enabled() {
                self.cached_http_document(target_url).await?
            } else {
                None
            };
            if let Some(cached) = cached_http_document.as_ref() {
                if cached.expires_at > chrono::Utc::now().timestamp() {
                    self.record_loaded_document(target_url)?;
                    self.record_document_metadata(target_url, cached.metadata.clone())?;
                    self.record_document(target_url, cached.document.clone())?;
                    return Ok(cached.document.clone());
                }
            }

            match self
                .fetch_http_external_document(target_url, cached_http_document.as_ref())
                .await?
            {
                OpenApiHttpDocumentFetch::NotModified => {
                    let cached = cached_http_document.as_ref().ok_or_else(|| {
                        StoreError::Other(format!(
                            "OpenAPI external $ref cache missing for 304 response: {target_url}"
                        ))
                    })?;
                    self.refresh_cached_http_document(target_url, cached)
                        .await?;
                    self.record_loaded_document(target_url)?;
                    self.record_document_metadata(target_url, cached.metadata.clone())?;
                    self.record_document(target_url, cached.document.clone())?;
                    return Ok(cached.document.clone());
                }
                OpenApiHttpDocumentFetch::Modified(response) => {
                    http_response = Some(response);
                }
            }
        } else if let Some(path) = &file_path {
            let document_text = read_openapi_document_file(path, target_url)?;
            let metadata = document_metadata_from_bytes(document_text.as_bytes());
            let fingerprint = file_document_fingerprint(path, target_url)?;
            if self.ref_cache.is_enabled() {
                if let Some((document, metadata)) = self
                    .cached_file_document(target_url, &metadata, &fingerprint)
                    .await?
                {
                    self.record_loaded_document(target_url)?;
                    self.record_document_metadata(target_url, metadata)?;
                    self.record_document(target_url, document.clone())?;
                    return Ok(document);
                }
            }
            file_metadata = Some(metadata);
            file_fingerprint = Some(fingerprint);
            file_document_text = Some(document_text);
        }

        let document_text = if let Some(response) = &http_response {
            response.text.clone()
        } else if let Some(document_text) = file_document_text {
            document_text
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
        let metadata = file_metadata
            .clone()
            .unwrap_or_else(|| document_metadata_from_bytes(document_text.as_bytes()));
        self.record_loaded_document(target_url)?;
        self.record_document_metadata(target_url, metadata.clone())?;
        self.record_document(target_url, document.clone())?;
        if is_http_url(target_url) && self.ref_cache.is_enabled() {
            let response = http_response.as_ref().ok_or_else(|| {
                StoreError::Other(format!("OpenAPI HTTP response missing for {target_url}"))
            })?;
            self.store_cached_http_document(target_url, &document, &metadata, response)
                .await?;
        } else if let Some(path) = &file_path {
            let fingerprint = file_fingerprint.as_ref().ok_or_else(|| {
                StoreError::Other(format!("OpenAPI file fingerprint missing for {target_url}"))
            })?;
            if self.ref_cache.is_enabled() {
                self.store_cached_file_document(
                    target_url,
                    path,
                    fingerprint,
                    &document,
                    &metadata,
                )
                .await?;
            }
        }
        Ok(document)
    }

    async fn fetch_http_external_document(
        &self,
        target_url: &str,
        cached: Option<&OpenApiCachedHttpDocument>,
    ) -> Result<OpenApiHttpDocumentFetch> {
        let mut request = self.client.get(target_url);
        if let Some(cached) = cached {
            if let Some(etag) = &cached.etag {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = &cached.last_modified {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
        let response = request.send().await.map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI external $ref fetch failed for {target_url}: {err}"
            ))
        })?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(OpenApiHttpDocumentFetch::NotModified);
        }
        let response = response.error_for_status().map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI external $ref fetch failed for {target_url}: {err}"
            ))
        })?;
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let last_modified = response
            .headers()
            .get(LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let text = response.text().await.map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI external $ref body read failed for {target_url}: {err}"
            ))
        })?;
        Ok(OpenApiHttpDocumentFetch::Modified(
            OpenApiHttpDocumentResponse {
                text,
                etag,
                last_modified,
            },
        ))
    }

    async fn cached_http_document(
        &self,
        target_url: &str,
    ) -> Result<Option<OpenApiCachedHttpDocument>> {
        let key = openapi_ref_document_cache_key(target_url);
        let Some(value) = self
            .cache
            .get_state(OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE, &key)
            .await?
        else {
            return Ok(None);
        };

        let Some(object) = value.as_object() else {
            return Ok(None);
        };
        if object
            .get("cache_version")
            .and_then(serde_json::Value::as_u64)
            != Some(OPENAPI_REF_DOCUMENT_CACHE_VERSION)
        {
            return Ok(None);
        }
        if object.get("url").and_then(serde_json::Value::as_str) != Some(target_url) {
            return Ok(None);
        }
        let Some(expires_at) = object.get("expires_at").and_then(serde_json::Value::as_i64) else {
            return Ok(None);
        };
        let Some(document) = object.get("document").cloned() else {
            return Ok(None);
        };
        let Some(content_hash) = object
            .get("content_hash")
            .and_then(serde_json::Value::as_str)
        else {
            return Ok(None);
        };
        let Some(content_length) = object
            .get("content_length")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            return Ok(None);
        };
        Ok(Some(OpenApiCachedHttpDocument {
            document,
            metadata: OpenApiBundleDocumentMetadata {
                content_hash: content_hash.to_string(),
                content_length,
            },
            expires_at,
            etag: object
                .get("etag")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            last_modified: object
                .get("last_modified")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
        }))
    }

    async fn store_cached_http_document(
        &self,
        target_url: &str,
        document: &serde_json::Value,
        metadata: &OpenApiBundleDocumentMetadata,
        response: &OpenApiHttpDocumentResponse,
    ) -> Result<()> {
        let key = openapi_ref_document_cache_key(target_url);
        let fetched_at = chrono::Utc::now().timestamp();
        let source = openapi_ref_document_source(target_url);
        self.cache
            .put_state(
                OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE,
                &key,
                serde_json::json!({
                    "cache_version": OPENAPI_REF_DOCUMENT_CACHE_VERSION,
                    "url": target_url,
                    "source": source,
                    "fetched_at": fetched_at,
                    "expires_at": fetched_at + self.ref_cache.ttl_seconds_i64(),
                    "ttl_seconds": self.ref_cache.ttl_seconds,
                    "content_hash": metadata.content_hash,
                    "content_length": metadata.content_length,
                    "etag": response.etag,
                    "last_modified": response.last_modified,
                    "document": document,
                }),
            )
            .await?;
        Ok(())
    }

    async fn refresh_cached_http_document(
        &self,
        target_url: &str,
        cached: &OpenApiCachedHttpDocument,
    ) -> Result<()> {
        let key = openapi_ref_document_cache_key(target_url);
        let fetched_at = chrono::Utc::now().timestamp();
        let source = openapi_ref_document_source(target_url);
        self.cache
            .put_state(
                OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE,
                &key,
                serde_json::json!({
                    "cache_version": OPENAPI_REF_DOCUMENT_CACHE_VERSION,
                    "url": target_url,
                    "source": source,
                    "fetched_at": fetched_at,
                    "expires_at": fetched_at + self.ref_cache.ttl_seconds_i64(),
                    "ttl_seconds": self.ref_cache.ttl_seconds,
                    "content_hash": cached.metadata.content_hash,
                    "content_length": cached.metadata.content_length,
                    "etag": cached.etag,
                    "last_modified": cached.last_modified,
                    "document": cached.document,
                }),
            )
            .await?;
        Ok(())
    }

    async fn cached_file_document(
        &self,
        target_url: &str,
        current_metadata: &OpenApiBundleDocumentMetadata,
        current_fingerprint: &OpenApiFileDocumentFingerprint,
    ) -> Result<Option<(serde_json::Value, OpenApiBundleDocumentMetadata)>> {
        let key = openapi_ref_document_cache_key(target_url);
        let Some(value) = self
            .cache
            .get_state(OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE, &key)
            .await?
        else {
            return Ok(None);
        };
        let Some(object) = value.as_object() else {
            return Ok(None);
        };
        if object
            .get("cache_version")
            .and_then(serde_json::Value::as_u64)
            != Some(OPENAPI_REF_DOCUMENT_CACHE_VERSION)
        {
            return Ok(None);
        }
        if object.get("url").and_then(serde_json::Value::as_str) != Some(target_url) {
            return Ok(None);
        }
        if object.get("source").and_then(serde_json::Value::as_str) != Some("file") {
            return Ok(None);
        }
        if object.get("file_size").and_then(serde_json::Value::as_u64)
            != Some(current_fingerprint.file_size)
        {
            return Ok(None);
        }
        if object
            .get("file_modified_unix_millis")
            .and_then(serde_json::Value::as_i64)
            != Some(current_fingerprint.file_modified_unix_millis)
        {
            return Ok(None);
        }
        let Some(document) = object.get("document").cloned() else {
            return Ok(None);
        };
        let Some(content_hash) = object
            .get("content_hash")
            .and_then(serde_json::Value::as_str)
        else {
            return Ok(None);
        };
        if content_hash != current_metadata.content_hash.as_str() {
            return Ok(None);
        }
        let Some(content_length) = object
            .get("content_length")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            return Ok(None);
        };
        if content_length != current_metadata.content_length {
            return Ok(None);
        }
        Ok(Some((
            document,
            OpenApiBundleDocumentMetadata {
                content_hash: content_hash.to_string(),
                content_length,
            },
        )))
    }

    async fn store_cached_file_document(
        &self,
        target_url: &str,
        path: &Path,
        fingerprint: &OpenApiFileDocumentFingerprint,
        document: &serde_json::Value,
        metadata: &OpenApiBundleDocumentMetadata,
    ) -> Result<()> {
        let key = openapi_ref_document_cache_key(target_url);
        let fetched_at = chrono::Utc::now().timestamp();
        self.cache
            .put_state(
                OPENAPI_REF_DOCUMENT_CACHE_STATE_TYPE,
                &key,
                serde_json::json!({
                    "cache_version": OPENAPI_REF_DOCUMENT_CACHE_VERSION,
                    "url": target_url,
                    "source": "file",
                    "path": path.to_string_lossy(),
                    "fetched_at": fetched_at,
                    "ttl_seconds": self.ref_cache.ttl_seconds,
                    "content_hash": metadata.content_hash,
                    "content_length": metadata.content_length,
                    "file_size": fingerprint.file_size,
                    "file_modified_unix_millis": fingerprint.file_modified_unix_millis,
                    "document": document,
                }),
            )
            .await?;
        Ok(())
    }

    fn record_dependency(
        &self,
        source_document: &str,
        source_ref: &str,
        target_document: &str,
        pointer: Option<&str>,
    ) -> Result<()> {
        self.dependencies
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI bundle dependency list lock poisoned".to_string())
            })?
            .push(OpenApiBundleDependency {
                source_document: document_label(source_document),
                source_ref: source_ref.to_string(),
                target_document: target_document.to_string(),
                pointer: pointer.map(ToString::to_string),
            });
        Ok(())
    }

    fn record_loaded_document(&self, target_url: &str) -> Result<()> {
        self.loaded_documents
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI bundle document list lock poisoned".to_string())
            })?
            .push(target_url.to_string());
        Ok(())
    }

    fn record_document_metadata(
        &self,
        target_url: &str,
        metadata: OpenApiBundleDocumentMetadata,
    ) -> Result<()> {
        self.document_metadata
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI bundle document metadata lock poisoned".to_string())
            })?
            .insert(target_url.to_string(), metadata);
        Ok(())
    }

    fn record_document(&self, target_url: &str, document: serde_json::Value) -> Result<()> {
        self.documents
            .lock()
            .map_err(|_| {
                StoreError::Other("OpenAPI external $ref cache lock poisoned".to_string())
            })?
            .insert(target_url.to_string(), document);
        Ok(())
    }
}

fn openapi_ref_document_cache_key(target_url: &str) -> String {
    format!("blake3:{}", blake3::hash(target_url.as_bytes()).to_hex())
}

fn openapi_ref_document_source(target_url: &str) -> String {
    reqwest::Url::parse(target_url)
        .map(|url| url.scheme().to_string())
        .unwrap_or_else(|_| "http".to_string())
}

fn document_metadata_from_value(
    value: &serde_json::Value,
) -> Result<OpenApiBundleDocumentMetadata> {
    let bytes = serde_json::to_vec(value).map_err(|err| {
        StoreError::Other(format!(
            "OpenAPI bundle document metadata serialization failed: {err}"
        ))
    })?;
    Ok(document_metadata_from_bytes(&bytes))
}

fn document_metadata_from_bytes(bytes: &[u8]) -> OpenApiBundleDocumentMetadata {
    OpenApiBundleDocumentMetadata {
        content_hash: format!("blake3:{}", blake3::hash(bytes).to_hex()),
        content_length: bytes.len(),
    }
}

fn file_document_fingerprint(path: &Path, label: &str) -> Result<OpenApiFileDocumentFingerprint> {
    let metadata = std::fs::metadata(path).map_err(|err| {
        StoreError::Other(format!(
            "OpenAPI file metadata read failed for {label}: {err}"
        ))
    })?;
    let modified = metadata.modified().map_err(|err| {
        StoreError::Other(format!(
            "OpenAPI file modified time read failed for {label}: {err}"
        ))
    })?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI file modified time is before Unix epoch for {label}: {err}"
            ))
        })?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| {
        StoreError::Other(format!(
            "OpenAPI file modified time is too large for {label}"
        ))
    })?;
    Ok(OpenApiFileDocumentFingerprint {
        file_size: metadata.len(),
        file_modified_unix_millis: millis,
    })
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

fn document_label(document_url: &str) -> String {
    normalize_document_target(document_url).unwrap_or_else(|_| document_url.to_string())
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
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "openapi_timeout_millis".to_string(),
            serde_json::Value::from(options.timeout_millis),
        );
        object.insert(
            "openapi_fetch_timeout_millis".to_string(),
            serde_json::Value::from(options.fetch_timeout_millis),
        );
    }
    Ok(value)
}
