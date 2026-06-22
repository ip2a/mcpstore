use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::cache::{CacheError, Result};

pub(super) fn value_to_object(value: Value) -> Result<HashMap<String, Value>> {
    match value {
        Value::Object(object) => Ok(object.into_iter().collect()),
        other => Err(CacheError::NotAnObject(format!("value={other}"))),
    }
}

pub(super) fn object_to_value(value: HashMap<String, Value>) -> Value {
    Value::Object(Map::from_iter(value))
}
