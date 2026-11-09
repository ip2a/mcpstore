use serde_json::{Map, Value};

pub fn merge_config(
    base: &Map<String, Value>,
    overrides: &Map<String, Value>,
) -> Map<String, Value> {
    let mut merged = base.clone();
    merge_object(&mut merged, overrides);
    merged
}

fn merge_object(target: &mut Map<String, Value>, overrides: &Map<String, Value>) {
    for (key, override_value) in overrides {
        if override_value.is_null() {
            target.remove(key);
            continue;
        }

        match (target.get_mut(key), override_value) {
            (Some(Value::Object(target_object)), Value::Object(override_object)) => {
                merge_object(target_object, override_object);
            }
            _ => {
                target.insert(key.clone(), override_value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn recursively_merges_objects_and_replaces_arrays_and_scalars() {
        let base = object(json!({
            "command": "uvx",
            "args": ["gitodo"],
            "env": {
                "TOKEN": "store-token",
                "REGION": "default"
            },
            "enabled": true
        }));
        let overrides = object(json!({
            "args": ["gitodo", "--mode", "agent1"],
            "env": {
                "TOKEN": "agent-token"
            },
            "enabled": false
        }));

        assert_eq!(
            Value::Object(merge_config(&base, &overrides)),
            json!({
                "command": "uvx",
                "args": ["gitodo", "--mode", "agent1"],
                "env": {
                    "TOKEN": "agent-token",
                    "REGION": "default"
                },
                "enabled": false
            })
        );
    }

    #[test]
    fn null_deletes_inherited_top_level_and_nested_fields() {
        let base = object(json!({
            "command": "uvx",
            "headers": {
                "Authorization": "Bearer store-token",
                "X-Tenant": "default"
            }
        }));
        let overrides = object(json!({
            "command": null,
            "headers": {
                "Authorization": null,
                "X-Tenant": "agent1"
            }
        }));

        assert_eq!(
            Value::Object(merge_config(&base, &overrides)),
            json!({
                "headers": {
                    "X-Tenant": "agent1"
                }
            })
        );
    }

    #[test]
    fn empty_values_are_not_treated_as_missing_or_delete_markers() {
        let base = object(json!({
            "args": ["one"],
            "description": "base",
            "env": {"TOKEN": "value"}
        }));
        let overrides = object(json!({
            "args": [],
            "description": "",
            "env": {}
        }));

        assert_eq!(
            Value::Object(merge_config(&base, &overrides)),
            json!({
                "args": [],
                "description": "",
                "env": {"TOKEN": "value"}
            })
        );
    }

    #[test]
    fn missing_fields_inherit_and_unknown_fields_are_preserved() {
        let base = object(json!({
            "command": "uvx",
            "futureTransportOption": {
                "mode": "strict"
            }
        }));

        assert_eq!(
            Value::Object(merge_config(&base, &Map::new())),
            json!({
                "command": "uvx",
                "futureTransportOption": {
                    "mode": "strict"
                }
            })
        );
    }
}
