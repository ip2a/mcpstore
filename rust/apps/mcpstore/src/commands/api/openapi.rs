use super::*;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(super) struct OpenApiImportRequest {
    spec_url: String,
    spec: Option<Value>,
    spec_text: Option<String>,
    timeout_millis: Option<u64>,
    fetch_timeout_millis: Option<u64>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    auth: serde_json::Map<String, Value>,
    #[serde(default)]
    ref_cache: OpenApiRefCachePolicy,
}

pub(super) async fn store_list_openapi_imports(State(state): State<Arc<ApiState>>) -> ApiResult {
    let imports = state
        .store
        .list_openapi_imports()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 导入列表获取成功",
        json!({ "imports": imports, "total": imports.len() }),
    ))
}

pub(super) async fn store_get_openapi_import_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
) -> ApiResult {
    let import = state
        .store
        .get_openapi_import(&name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 导入结果获取成功",
        json!({ "import": import }),
    ))
}

pub(super) async fn store_import_openapi_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiImportOptions {
        headers: payload.headers,
        auth: payload.auth,
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(payload.timeout_millis, "timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_timeout_millis),
        fetch_timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .unwrap_or_else(OpenApiImportOptions::default_fetch_timeout_millis),
    };
    store_import_openapi_impl(
        state,
        name,
        payload.spec_url,
        payload.spec,
        payload.spec_text,
        options,
    )
    .await
}

pub(super) async fn store_import_openapi_impl(
    state: Arc<ApiState>,
    name: String,
    spec_url: String,
    spec: Option<Value>,
    spec_text: Option<String>,
    options: OpenApiImportOptions,
) -> ApiResult {
    let import = match (spec, payload_spec_text(spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .import_openapi_service_from_spec_with_options(&name, &spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .import_openapi_service_from_spec_text_with_options(
                    &name, &spec_url, &spec_text, options,
                )
                .await
        }
        (None, None) => {
            state
                .store
                .import_openapi_service_with_options(&name, &spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success("OpenAPI 导入成功", json!({ "import": import })))
}

pub(super) async fn store_bundle_openapi(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiBundleOptions {
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .or(openapi_timeout_millis(
            payload.timeout_millis,
            "timeout_millis",
        )?)
        .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    };
    let bundle = match (payload.spec, payload_spec_text(payload.spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .bundle_openapi_spec_from_value_with_options(&payload.spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .bundle_openapi_spec_from_text_with_options(&payload.spec_url, &spec_text, options)
                .await
        }
        (None, None) => {
            state
                .store
                .bundle_openapi_spec_with_options(&payload.spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success("OpenAPI 规范打包成功", json!({ "bundle": bundle })))
}

pub(super) async fn store_bundle_openapi_artifact(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiBundleOptions {
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .or(openapi_timeout_millis(
            payload.timeout_millis,
            "timeout_millis",
        )?)
        .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    };
    let artifact = match (payload.spec, payload_spec_text(payload.spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .bundle_openapi_artifact_from_value_with_options(&payload.spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .bundle_openapi_artifact_from_text_with_options(
                    &payload.spec_url,
                    &spec_text,
                    options,
                )
                .await
        }
        (None, None) => {
            state
                .store
                .bundle_openapi_artifact_with_options(&payload.spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 规范打包产物获取成功",
        json!({ "artifact": artifact }),
    ))
}

fn payload_spec_text(spec_text: Option<String>) -> Option<String> {
    spec_text.filter(|text| !text.trim().is_empty())
}

fn openapi_timeout_millis(value: Option<u64>, field: &'static str) -> ApiResult<Option<u64>> {
    match value {
        Some(0) => Err(ApiError::invalid_parameter(
            format!("{field} must be a positive integer"),
            Some(field),
        )),
        other => Ok(other),
    }
}
