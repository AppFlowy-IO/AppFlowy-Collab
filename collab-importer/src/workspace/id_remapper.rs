use serde_json;
use std::collections::HashMap;
use uuid::Uuid;

/// Common utilities for remapping IDs in JSON structures
pub struct JsonIdRemapper<'a> {
  id_mapping: &'a HashMap<String, String>,
}

impl<'a> JsonIdRemapper<'a> {
  pub fn new(id_mapping: &'a HashMap<String, String>) -> Self {
    Self { id_mapping }
  }

  /// Recursively remap all IDs in a JSON value
  pub fn remap_json_value(&self, value: &mut serde_json::Value) {
    match value {
      serde_json::Value::Object(map) => {
        for (_key, val) in map.iter_mut() {
          self.remap_json_value(val);
        }
      },
      serde_json::Value::Array(arr) => {
        for item in arr.iter_mut() {
          self.remap_json_value(item);
        }
      },
      serde_json::Value::String(s) => {
        if is_uuid(s) {
          *s = self.map_id(s);
        } else {
          *s = self.remap_uuids_in_string(s);
        }
      },
      _ => {},
    }
  }

  /// Map an old ID to a new ID, or return the old ID if no mapping exists
  pub fn map_id(&self, old_id: &str) -> String {
    self
      .id_mapping
      .get(old_id)
      .cloned()
      .unwrap_or_else(|| old_id.to_string())
  }

  /// Replace all UUID occurrences in a string
  pub fn remap_uuids_in_string(&self, s: &str) -> String {
    let mut result = s.to_string();

    for (old_id, new_id) in self.id_mapping {
      if result.contains(old_id) {
        result = result.replace(old_id, new_id);
      }
    }

    result
  }
}

/// Check if a string is a valid UUID
pub fn is_uuid(s: &str) -> bool {
  Uuid::parse_str(s).is_ok()
}
