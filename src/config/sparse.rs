use serde_json::{Map, Value};

use super::{AppConfig, Result};

/// Recursively diff config against default values.
/// Returns None when nothing differs (or only nulls were dropped).
fn diff_value(config: &Value, default: Option<&Value>) -> Option<Value> {
    // TOML has no null; None means "unset" (omit the key, like the previous
    // direct struct serializer did).
    if config.is_null() {
        return None;
    }

    match (config, default) {
        (Value::Object(config_obj), Some(Value::Object(default_obj))) => {
            let mut out = Map::new();
            for (key, cfg_val) in config_obj {
                if let Some(diff) = diff_value(cfg_val, default_obj.get(key)) {
                    out.insert(key.clone(), diff);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(Value::Object(out))
            }
        }
        (Value::Object(config_obj), None) => {
            // No default to compare against; strip nulls, keep structure
            let mut out = Map::new();
            for (key, cfg_val) in config_obj {
                if let Some(diff) = diff_value(cfg_val, None) {
                    out.insert(key.clone(), diff);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(Value::Object(out))
            }
        }
        (cfg, Some(def)) if cfg == def => None,
        (cfg, _) => Some(cfg.clone()),
    }
}

/// Compute the difference between config and default values.
/// Only returns fields that differ from their defaults, enabling sparse persistence.
pub fn diff_from_default(config: &AppConfig) -> Result<Value> {
    let default = AppConfig::default();
    let config_value = serde_json::to_value(config)?;
    let default_value = serde_json::to_value(&default)?;

    Ok(diff_value(&config_value, Some(&default_value)).unwrap_or(Value::Object(Map::new())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_only_includes_changed_fields() {
        let mut config = AppConfig::default();
        // Change only one field
        config.server.port = 1830;

        let diff = diff_from_default(&config).unwrap();
        let diff_obj = diff.as_object().unwrap();

        // Should have server section (because port changed)
        assert!(diff_obj.contains_key("server"));

        // Should NOT have unchanged sections
        assert!(!diff_obj.contains_key("cache"));
        assert!(!diff_obj.contains_key("api"));
    }

    #[test]
    fn diff_of_default_config_is_empty() {
        let config = AppConfig::default(); // Everything is default
        let diff = diff_from_default(&config).unwrap();

        // No user intent, nothing written
        assert!(diff.as_object().unwrap().is_empty());
    }
}
