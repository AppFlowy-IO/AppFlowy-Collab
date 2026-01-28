use serde_json::{self, Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub struct JsonIdRemapper<'a> {
  id_mapping: &'a HashMap<String, String>,
}

impl<'a> JsonIdRemapper<'a> {
  pub fn new(id_mapping: &'a HashMap<String, String>) -> Self {
    Self { id_mapping }
  }

  pub fn remap_json_value(&self, value: &mut Value) {
    match value {
      Value::Object(map) => {
        self.remap_json_keys(map);

        for (_key, val) in map.iter_mut() {
          self.remap_json_value(val);
        }
      },
      Value::Array(arr) => {
        for item in arr.iter_mut() {
          self.remap_json_value(item);
        }
      },
      Value::String(s) => {
        if Self::is_uuid(s) {
          *s = self.map_id(s);
        } else {
          *s = self.remap_uuids_in_string(s);
        }
      },
      _ => {},
    }
  }

  pub fn map_id(&self, old_id: &str) -> String {
    self
      .id_mapping
      .get(old_id)
      .cloned()
      .unwrap_or_else(|| old_id.to_string())
  }

  pub fn remap_uuids_in_string(&self, s: &str) -> String {
    let mut result = s.to_string();

    for (old_id, new_id) in self.id_mapping {
      if result.contains(old_id) {
        result = result.replace(old_id, new_id);
      }
    }

    result
  }

  pub fn remap_json_keys(&self, map: &mut Map<String, Value>) {
    let mut remap_pairs = Vec::with_capacity(map.len());
    for key in map.keys() {
      if Self::is_uuid(key) {
        let new_key = self.map_id(key);
        if new_key != *key {
          remap_pairs.push((key.clone(), new_key));
        }
      }
    }

    for (old_key, new_key) in remap_pairs {
      if let Some(value) = map.remove(&old_key) {
        map.insert(new_key, value);
      }
    }
  }

  fn is_uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
  }
}
