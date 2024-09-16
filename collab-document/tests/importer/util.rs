use collab_document::blocks::{Block, DocumentData};
use collab_document::importer::md_importer::MDImporter;
use serde_json::Value;

pub(crate) fn markdown_to_document_data(md: &str) -> DocumentData {
  let importer = MDImporter::new(None);
  let result = importer.import("test_document", md);
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

pub(crate) fn get_children_blocks(document_data: &DocumentData, block_id: &str) -> Vec<Block> {
  let block = get_block(document_data, block_id);
  let children_ids = document_data.meta.children_map.get(&block.id).unwrap();
  children_ids
    .iter()
    .map(|id| get_block(document_data, id))
    .collect()
}

pub(crate) fn get_delta(document_data: &DocumentData, block_id: &str) -> String {
  let block = get_block(document_data, block_id);
  let delta = document_data
    .meta
    .text_map
    .as_ref()
    .unwrap()
    .get(&block.id)
    .unwrap();
  delta.clone()
}
