use collab::core::collab::CollabOptions;
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use std::collections::HashMap;

use crate::blocks::{Block, DocumentData, DocumentMeta, TextDelta};
use crate::document::Document;
use crate::error::DocumentError;

const INLINE_DATABASE_BLOCK_TYPES: &[&str] = &["grid", "board", "calendar"];
const PARENT_ID_KEY: &str = "parent_id";
const VIEW_ID_KEY: &str = "view_id";

const MENTION_KEY: &str = "mention";
const MENTION_PAGE_ID_KEY: &str = "page_id";

pub struct DocumentCollabRemapper {
  id_mapping: HashMap<String, String>,
}

impl DocumentCollabRemapper {
  pub fn new(id_mapping: HashMap<String, String>) -> Self {
    Self { id_mapping }
  }

  // remap the collab document
  //
  // 1. replace all the inline database page id
  // 2. replace all the mentioned page id
  // 3. replace all the reference page (not supported yet)
  // 4. replace all the image url (not supported yet)
  // 5. replace all the file url (not supported yet)
  pub fn remap_collab_doc(
    &self,
    doc_id: &str,
    user_id: &str,
    doc: Document,
  ) -> Result<Document, DocumentError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);
    let document_data = doc.get_document_data()?;
    let remapped_data = self.remap_document_data(document_data)?;

    let new_options = CollabOptions::new(doc_id.to_string(), client_id);
    let new_collab = Collab::new_with_options(CollabOrigin::Empty, new_options)
      .map_err(|e| DocumentError::Internal(anyhow::Error::new(e)))?;
    let new_document = Document::create_with_data(new_collab, remapped_data)?;

    Ok(new_document)
  }

  pub fn remap_collab_doc_state(
    &self,
    doc_id: &str,
    user_id: &str,
    doc_state: &Vec<u8>,
  ) -> Result<Vec<u8>, DocumentError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);
    let options = CollabOptions::new(doc_id.to_string(), client_id)
      .with_data_source(DataSource::DocStateV1(doc_state.clone()));
    let collab = Collab::new_with_options(CollabOrigin::Empty, options)
      .map_err(|e| DocumentError::Internal(anyhow::Error::new(e)))?;
    let document = Document::open(collab)?;
    let new_document = self.remap_collab_doc(doc_id, user_id, document)?;
    let updated_state = new_document.encode_collab()?;
    Ok(updated_state.doc_state.to_vec())
  }

  fn remap_document_data(
    &self,
    document_data: DocumentData,
  ) -> Result<DocumentData, DocumentError> {
    let remapped_blocks = self.remap_blocks(document_data.blocks);
    let remapped_text_map = self.remap_text_map(document_data.meta.text_map);

    let remapped_meta = DocumentMeta {
      children_map: document_data.meta.children_map,
      text_map: remapped_text_map,
    };

    Ok(DocumentData {
      page_id: document_data.page_id,
      blocks: remapped_blocks,
      meta: remapped_meta,
    })
  }

  fn remap_blocks(&self, blocks: HashMap<String, Block>) -> HashMap<String, Block> {
    blocks
      .into_iter()
      .map(|(block_id, block)| {
        let remapped_block = self.remap_block(block);
        (block_id, remapped_block)
      })
      .collect()
  }

  fn remap_block(&self, mut block: Block) -> Block {
    if INLINE_DATABASE_BLOCK_TYPES.contains(&block.ty.as_str()) {
      if let Some(parent_id) = block.data.get(PARENT_ID_KEY).and_then(|v| v.as_str()) {
        if let Some(view_id) = block.data.get(VIEW_ID_KEY).and_then(|v| v.as_str()) {
          if !parent_id.is_empty() && !view_id.is_empty() {
            if let Some(new_parent_id) = self.id_mapping.get(parent_id) {
              if let Some(new_view_id) = self.id_mapping.get(view_id) {
                block
                  .data
                  .insert(PARENT_ID_KEY.to_string(), new_parent_id.clone().into());
                block
                  .data
                  .insert(VIEW_ID_KEY.to_string(), new_view_id.clone().into());
              }
            }
          }
        }
      }
    }

    block
  }

  fn remap_text_map(
    &self,
    text_map: Option<HashMap<String, String>>,
  ) -> Option<HashMap<String, String>> {
    text_map.map(|map| {
      map
        .into_iter()
        .map(|(text_id, text_delta_str)| {
          let remapped_delta_str = self.remap_text_delta_string(text_delta_str);
          (text_id, remapped_delta_str)
        })
        .collect()
    })
  }

  fn remap_text_delta_string(&self, text_delta_str: String) -> String {
    if let Ok(text_deltas) = serde_json::from_str::<Vec<TextDelta>>(&text_delta_str) {
      if let Ok(Some(updated_deltas)) = self.remap_mention_ids_in_text_deltas(text_deltas) {
        if let Ok(new_delta_str) = serde_json::to_string(&updated_deltas) {
          return new_delta_str;
        }
      }
    }

    text_delta_str
  }

  fn remap_mention_ids_in_text_deltas(
    &self,
    text_deltas: Vec<TextDelta>,
  ) -> Result<Option<Vec<TextDelta>>, DocumentError> {
    let mut updated_deltas = Vec::new();
    let mut has_changes = false;

    for delta in text_deltas {
      match delta {
        TextDelta::Inserted(text, Some(attributes)) => {
          let mut new_attributes = attributes.clone();

          if let Some(mention_data) = attributes.get(MENTION_KEY) {
            if let Some(updated_mention_str) = self.remap_mention_object_to_string(mention_data)? {
              new_attributes.insert(MENTION_KEY.into(), updated_mention_str.into());
              has_changes = true;
            }
          }

          updated_deltas.push(TextDelta::Inserted(text, Some(new_attributes)));
        },
        _ => updated_deltas.push(delta),
      }
    }

    if has_changes {
      Ok(Some(updated_deltas))
    } else {
      Ok(None)
    }
  }

  fn remap_mention_object_to_string(
    &self,
    mention_data: &collab::preclude::Any,
  ) -> Result<Option<String>, DocumentError> {
    let mention_str = serde_json::to_string(mention_data)
      .map_err(|e| DocumentError::Internal(anyhow::Error::new(e)))?;
    let mut mention_map: serde_json::Map<String, serde_json::Value> =
      match serde_json::from_str(&mention_str) {
        Ok(map) => map,
        Err(_) => return Ok(None),
      };

    if let Some(page_id) = mention_map
      .get(MENTION_PAGE_ID_KEY)
      .and_then(|v| v.as_str())
    {
      if let Some(new_page_id) = self.id_mapping.get(page_id) {
        mention_map.insert(MENTION_PAGE_ID_KEY.to_string(), new_page_id.clone().into());
        let updated_str = serde_json::to_string(&mention_map)
          .map_err(|e| DocumentError::Internal(anyhow::Error::new(e)))?;
        return Ok(Some(updated_str));
      }
    }

    Ok(None)
  }
}
