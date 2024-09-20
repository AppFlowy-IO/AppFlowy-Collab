use collab_document::blocks::{Block, DocumentData};
use collab_document::importer::md_importer::MDImporter;
use serde_json::Value;

pub(crate) fn markdown_to_document_data<T: ToString>(md: T) -> DocumentData {
  let importer = MDImporter::new(None);
  let result = importer.import("test_document", md.to_string());
  result.unwrap()
}

pub(crate) fn parse_json(s: &str) -> Value {
  serde_json::from_str(s).unwrap()
}

pub(crate) fn get_page_block(document_data: &DocumentData) -> Block {
  document_data
    .blocks
    .values()
    .find(|b| b.ty == "page")
    .unwrap()
    .clone()
}

pub(crate) fn get_block(document_data: &DocumentData, block_id: &str) -> Block {
  document_data.blocks.get(block_id).unwrap().clone()
}

pub(crate) fn get_block_by_type(document_data: &DocumentData, block_type: &str) -> Block {
  document_data
    .blocks
    .values()
    .find(|b| b.ty == block_type)
    .unwrap()
    .clone()
}

pub(crate) fn get_children_blocks(document_data: &DocumentData, block_id: &str) -> Vec<Block> {
  let block = get_block(document_data, block_id);
  let children_ids = document_data.meta.children_map.get(&block.id).unwrap();
  children_ids
    .iter()
    .map(|id| get_block(document_data, id))
    .collect()
}

pub(crate) fn get_delta(document_data: &DocumentData, block_id: &str) -> String {
  let delta = document_data
    .meta
    .text_map
    .as_ref()
    .unwrap()
    .get(block_id)
    .unwrap();
  delta.clone()
}

pub(crate) fn get_delta_json(document_data: &DocumentData, block_id: &str) -> Value {
  let delta = get_delta(document_data, block_id);
  parse_json(&delta)
}

// Prints all child blocks of the page block for debugging purposes.
#[allow(dead_code)]
pub(crate) fn dump_page_blocks(document_data: &DocumentData) {
  let page_block = get_page_block(document_data);
  let children_blocks = get_children_blocks(document_data, &page_block.id);
  for block in children_blocks {
    println!("{:?}", block);
  }
}
