use openkeyv::{StructuredValue, Value};
use serde_json::Value as JsonValue;

use crate::cache::{CacheError, Result};

pub(crate) fn json_to_value(json: JsonValue) -> Result<Value> {
    let structured = StructuredValue::from_json(&json).map_err(map_openkeyv_err)?;
    Value::from_structured(&structured).map_err(map_openkeyv_err)
}

pub(crate) fn value_to_json(value: Value) -> Result<JsonValue> {
    let structured = value.decode_structured().map_err(map_openkeyv_err)?;
    structured.to_json().map_err(map_openkeyv_err)
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv value conversion failed: {err}"))
}
