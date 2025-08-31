use anyhow::Result;
use collab::core::collab::default_client_id;
use collab_document::blocks::DocumentData;
use collab_document::document::Document;
use serde_json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::workspace::id_remapper::JsonIdRemapper;

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
    let remapper = JsonIdRemapper::new(&self.id_mapping);
    remapper.remap_json_value(&mut json_value);
    Ok(json_value)
  }

  pub fn build_document_data(&self) -> Result<DocumentData> {
    let remapped_json = self.remap_json()?;
    let document_data: DocumentData = serde_json::from_value(remapped_json)?;
    Ok(document_data)
  }

  pub fn build_document(&self, document_id: &Uuid) -> Result<Document> {
    let document_data = self.build_document_data()?;
    let document = Document::create(&document_id.to_string(), document_data, default_client_id())?;
    Ok(document)
  }
}
