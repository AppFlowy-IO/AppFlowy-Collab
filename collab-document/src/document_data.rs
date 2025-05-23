use collab::preclude::ClientID;
use std::collections::HashMap;

use crate::blocks::{Block, DocumentData, DocumentMeta};
use crate::document::Document;
use crate::error::DocumentError;
use collab::entity::EncodedCollab;
use nanoid::nanoid;
use uuid::Uuid;

pub const PAGE: &str = "page";
pub const PARAGRAPH_BLOCK_TYPE: &str = "paragraph";

/// Generates default data for a document.
///
/// This function constructs a `DocumentData` instance that includes a page block and a text block.
/// Each block has a unique identifier, generated using the nanoid crate. The page block is set as the
/// parent of the text block.
///
/// The `DocumentData` struct has three main components:
/// - `page_id`: a unique identifier for the root page block.
/// - `blocks`: a `HashMap` where each key-value pair represents a block. The key is the unique
///    identifier of the block, and the value is the block data itself.
/// - `meta`: a `DocumentMeta` instance which contains a `children_map`. This `HashMap` represents
///    the parent-child relationships of the blocks. Each key-value pair consists of a block id and
///    a vector of ids of its children.
///
/// # Returns
/// A `DocumentData` instance populated with a single page block and a single child text block.
///
pub fn default_document_data(document_id: &str) -> DocumentData {
  let page_type = PAGE.to_string();
  let text_type = PARAGRAPH_BLOCK_TYPE.to_string();

  let mut blocks: HashMap<String, Block> = HashMap::new();
  let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
  let mut text_map: HashMap<String, String> = HashMap::new();

  // page block
  let page_id = page_id_from_document_id(document_id).unwrap_or_else(generate_id);
  let children_id = page_id.clone();
  let root = Block {
    id: page_id.clone(),
    ty: page_type,
    parent: "".to_string(),
    children: children_id.clone(),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  blocks.insert(page_id.clone(), root);

  // text block
  let text_block_id = generate_id();
  let text_block_children_id = generate_id();
  let text_external_id = generate_id();
  let text_block = Block {
    id: text_block_id.clone(),
    ty: text_type,
    parent: page_id.clone(),
    children: text_block_children_id.clone(),
    external_id: Some(text_external_id.clone()),
    external_type: Some("text".to_string()),
    data: HashMap::new(),
  };
  blocks.insert(text_block_id.clone(), text_block);

  // children_map
  children_map.insert(children_id, vec![text_block_id]);
  children_map.insert(text_block_children_id, vec![]);

  // text_map
  text_map.insert(text_external_id, "[]".to_string());

  DocumentData {
    page_id,
    blocks,
    meta: DocumentMeta {
      children_map,
      text_map: Some(text_map),
    },
  }
}

/// Generates default collab data for a document. This document only contains the initial state
/// of the document.
pub fn default_document_collab_data(
  document_id: &str,
  client_id: ClientID,
) -> Result<EncodedCollab, DocumentError> {
  let document_data = default_document_data(document_id);
  let document = Document::create(document_id, document_data, client_id)?;
  document.encode_collab()
}

pub fn generate_id() -> String {
  nanoid!(10)
}

pub fn page_id_from_document_id(document_id: &str) -> Option<String> {
  let document_uuid = Uuid::parse_str(document_id).ok()?;
  Some(Uuid::new_v5(&document_uuid, PAGE.as_bytes()).to_string())
}
