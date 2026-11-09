use std::collections::HashMap;

use serde_json::Value;

pub(super) fn flatten_config_value(value: &Value) -> HashMap<String, Value> {
    let mut out = HashMap::new();

    fn visit(prefix: &str, value: &Value, out: &mut HashMap<String, Value>) {
        match value {
            Value::Object(map) => {
                for (key, item) in map {
                    let next = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    visit(&next, item, out);
                }
            }
            other => {
                if !prefix.is_empty() {
                    out.insert(prefix.to_string(), other.clone());
                }
            }
        }
    }

    visit("", value, &mut out);
    out
}
