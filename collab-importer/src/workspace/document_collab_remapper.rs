use anyhow::Result;
use collab_document::blocks::DocumentData;
use collab_document::document::Document;
use serde_json;
use std::collections::HashMap;
use uuid::Uuid;

pub struct DocumentCollabRemapper {
  id_mapping: HashMap<String, String>,
  document_json: serde_json::Value,
}

impl DocumentCollabRemapper {
  pub fn new(document_json: serde_json::Value, id_mapping: HashMap<String, String>) -> Self {
    Self {
      id_mapping,
      document_json,
    }
  }

  pub fn remap_json(&self) -> Result<serde_json::Value> {
    let mut json_value = self.document_json.clone();
    self.remap_json_value(&mut json_value);
    Ok(json_value)
  }

  pub fn build_document_data(&self) -> Result<DocumentData> {
    let remapped_json = self.remap_json()?;
    let document_data: DocumentData = serde_json::from_value(remapped_json)?;
    Ok(document_data)
  }

  pub fn build_document(&self, document_id: &str) -> Result<Document> {
    let document_data = self.build_document_data()?;
    let document = Document::create(document_id, document_data, 1)?;
    Ok(document)
  }

  fn remap_json_value(&self, value: &mut serde_json::Value) {
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
        if self.is_uuid(s) {
          *s = self.map_view_id(s);
        } else {
          *s = self.remap_uuids_in_string(s);
        }
      },
      _ => {},
    }
  }

  fn is_uuid(&self, s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
  }

  fn map_view_id(&self, old_id: &str) -> String {
    self
      .id_mapping
      .get(old_id)
      .cloned()
      .unwrap_or_else(|| old_id.to_string())
  }

  fn remap_uuids_in_string(&self, s: &str) -> String {
    let mut result = s.to_string();

    for (old_id, new_id) in &self.id_mapping {
      if result.contains(old_id) {
        result = result.replace(old_id, new_id);
      }
    }

    result
  }
}
