// ~/src/data_loaders/data_utils.rs

use std::path::Path;

// YAML Data Utilities
// --------------------------------------------

// Check if automatic config updates are enabled for a given yaml file
pub fn is_yaml_update_check_enabled(path: &Path) -> bool {
    use crate::data_loaders::yaml::load_yaml;
    if let Some(v) = load_yaml(path) {
        if let Some(enabled) = v.get("update_check").and_then(|v| v.as_bool()) {
            return enabled;
        }
    }
    false
}

// Parse YAML into a HashMap
pub fn parse_yaml(path: &Path) -> Option<std::collections::HashMap<String, serde_yaml::Value>> {
    use crate::data_loaders::yaml::load_yaml;
    if let Some(v) = load_yaml(path) {
        if let Some(map) = v.as_mapping() {
            let mut result = std::collections::HashMap::new();
            for (k, val) in map {
                if let Some(key_str) = k.as_str() {
                    result.insert(key_str.to_string(), val.clone());
                }
            }
            return Some(result);
        }
    }
    None
}
// --------------------------------------------



// JSON Data Utilities
// --------------------------------------------

// Check if automatic config updates are enabled for a given json file
pub fn is_json_update_check_enabled(path: &Path) -> bool {
    use crate::data_loaders::json::load_json;
    if let Some(v) = load_json(path) {
        if let Some(enabled) = v.get("update_check").and_then(|v| v.as_bool()) {
            return enabled;
        }
    }
    false
}

// Parse JSON into a HashMap
pub fn parse_json(path: &Path) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    use crate::data_loaders::json::load_json;
    if let Some(v) = load_json(path) {
        if let Some(map) = v.as_object() {
            let mut result = std::collections::HashMap::new();
            for (k, val) in map {
                result.insert(k.clone(), val.clone());
            }
            return Some(result);
        }
    }
    None
}
// --------------------------------------------
