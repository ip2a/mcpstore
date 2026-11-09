pub(crate) fn wrap_cache_item(
    key: &str,
    type_name: &str,
    collection: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    let mut object = match value {
        serde_json::Value::Object(object) => object,
        other => {
            let mut object = serde_json::Map::new();
            object.insert("value".to_string(), other);
            object
        }
    };
    object.insert("_key".to_string(), serde_json::json!(key));
    object.insert("_type".to_string(), serde_json::json!(type_name));
    object.insert("_collection".to_string(), serde_json::json!(collection));
    serde_json::Value::Object(object)
}
