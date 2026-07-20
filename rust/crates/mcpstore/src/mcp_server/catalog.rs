use super::tools::{read_required_instance_id, read_required_string};
use super::*;

pub(super) fn project_prompt_names(mut payloads: Vec<Value>) -> Result<Vec<Value>, ErrorData> {
    let names = catalog_name_counts(&payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in &mut payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if names.get(&original).copied().unwrap_or_default() < 2 {
            continue;
        }
        let service_name = read_required_string(payload, "service_name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("prompt must be an object", None))?
            .insert(
                "name".to_string(),
                Value::String(format!(
                    "{}__{}",
                    stable_namespace(&service_name, instance_id),
                    original
                )),
            );
    }
    Ok(payloads)
}

pub(super) fn resolve_projected_prompt(
    payloads: &[Value],
    projected_name: &str,
) -> Result<(InstanceId, String), ErrorData> {
    let names = catalog_name_counts(payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let candidate = if names.get(&original).copied().unwrap_or_default() > 1 {
            let service_name = read_required_string(payload, "service_name")
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
            format!(
                "{}__{}",
                stable_namespace(&service_name, instance_id),
                original
            )
        } else {
            original.clone()
        };
        if candidate == projected_name {
            return Ok((instance_id, original));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown aggregate prompt: {projected_name}"),
        None,
    ))
}

pub(super) fn catalog_name_counts(
    payloads: &[Value],
    field: &str,
) -> Result<HashMap<String, usize>, BoxErr> {
    let mut names = HashMap::new();
    for payload in payloads {
        *names
            .entry(read_required_string(payload, field)?)
            .or_default() += 1;
    }
    Ok(names)
}

pub(super) fn project_catalog_uris(
    mut payloads: Vec<Value>,
    field: &str,
    template: bool,
) -> Result<Vec<Value>, ErrorData> {
    for payload in &mut payloads {
        let projected = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let object = payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("catalog item must be an object", None))?;
        object.insert(field.to_string(), Value::String(projected));
    }
    Ok(payloads)
}

pub(super) fn resolve_projected_catalog_uri(
    payloads: &[Value],
    field: &str,
    template: bool,
    projected_uri: &str,
) -> Result<(InstanceId, String), ErrorData> {
    for payload in payloads {
        let candidate = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if candidate == projected_uri {
            return Ok((
                read_required_instance_id(payload, "instance_id")
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
                read_required_string(payload, field)
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
            ));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown aggregate resource URI: {projected_uri}"),
        None,
    ))
}

pub(super) fn projected_catalog_uri(
    payload: &Value,
    field: &str,
    template: bool,
) -> Result<String, BoxErr> {
    let original = read_required_string(payload, field)?;
    let service_name = read_required_string(payload, "service_name")?;
    let instance_id = read_required_instance_id(payload, "instance_id")?;
    let namespace = stable_namespace(&service_name, instance_id);
    let mut uri = reqwest::Url::parse("mcpstore://aggregate/")?;
    {
        let mut segments = uri
            .path_segments_mut()
            .map_err(|_| "aggregate URI cannot contain path segments")?;
        segments.push(&namespace);
        if template {
            segments.push("template");
        }
        segments.push(&original);
    }
    Ok(uri.into())
}

pub(super) fn stable_namespace(service_name: &str, instance_id: InstanceId) -> String {
    let service_name = service_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{service_name}__{instance_id}")
}
