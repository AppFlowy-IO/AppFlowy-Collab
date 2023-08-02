use crate::blocks::{Block, DocumentData, DocumentMeta};
use nanoid::nanoid;
use std::collections::HashMap;

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
pub fn default_document_data() -> DocumentData {
  let page_type = PAGE.to_string();
  let text_type = PARAGRAPH_BLOCK_TYPE.to_string();

  let mut blocks: HashMap<String, Block> = HashMap::new();
  let mut meta: HashMap<String, Vec<String>> = HashMap::new();

  // page block
  let page_id = nanoid!(10);
  let children_id = nanoid!(10);
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
  let text_block_id = nanoid!(10);
  let text_block_children_id = nanoid!(10);
  let text_block = Block {
    id: text_block_id.clone(),
    ty: text_type,
    parent: page_id.clone(),
    children: text_block_children_id.clone(),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  blocks.insert(text_block_id.clone(), text_block);

  // meta
  meta.insert(children_id, vec![text_block_id]);
  meta.insert(text_block_children_id, vec![]);

  DocumentData {
    page_id,
    blocks,
    meta: DocumentMeta { children_map: meta },
  }
}

/// The default document data.
pub fn default_document_data2() -> DocumentData {
  let page_type = PAGE.to_string();
  let text_type = PARAGRAPH_BLOCK_TYPE.to_string();

  let mut blocks: HashMap<String, Block> = HashMap::new();
  let mut meta: HashMap<String, Vec<String>> = HashMap::new();

  // page block
  let page_id = nanoid!(10);
  let children_id = nanoid!(10);
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
  let text_block_id = nanoid!(10);
  let text_block_children_id = nanoid!(10);
  let text_block = Block {
    id: text_block_id.clone(),
    ty: text_type,
    parent: page_id.clone(),
    children: text_block_children_id.clone(),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  blocks.insert(text_block_id.clone(), text_block);

  // meta
  meta.insert(children_id, vec![text_block_id]);
  meta.insert(text_block_children_id, vec![]);
  DocumentData {
    page_id,
    blocks,
    meta: DocumentMeta { children_map: meta },
  }
}
