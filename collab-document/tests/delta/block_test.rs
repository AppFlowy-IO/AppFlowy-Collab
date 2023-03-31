use crate::util::{create_document, delete_block, inser_text_block};
use collab_document::blocks::{BlockDataParser, DataParser};

#[test]
fn create_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let document_data = test.document.to_json().unwrap();
  let document = &document_data["document"];

  let root_id = document["root_id"].as_str().unwrap();
  let blocks = &document["blocks"];
  let meta = &document["meta"];
  let text_map = &meta["text_map"];
  let children_map = &meta["children_map"];

  assert!(blocks.is_object());
  assert!(text_map.is_object());
  assert!(children_map.is_object());

  assert!(text_map.as_object().unwrap().len() == 2);
  assert!(children_map.as_object().unwrap().len() == 2);

  let root = &blocks[root_id];
  let root_data = &root["data"];
  let root_children = root["children"].as_str().unwrap();
  let root_text = root_data["text"].as_str().unwrap();

  assert!(root["ty"] == "page");
  assert!(children_map[root_children].is_array());
  assert!(text_map[root_text].is_array());
  assert!(children_map[root_children].as_array().unwrap().len() == 1);

  let head_id = children_map[root_children].as_array().unwrap()[0]
    .as_str()
    .unwrap();
  let head = blocks[head_id].as_object().unwrap();
  let head_data = head["data"].as_object().unwrap();
  let head_children = head["children"].as_str().unwrap();
  let head_text = head_data["text"].as_str().unwrap();
  assert!(head["ty"] == "text");
  assert!(children_map[head_children].is_array());
  assert!(text_map[head_text].is_array());
  assert!(children_map[head_children].as_array().unwrap().len() == 0);
  assert!(children_map[root_children][0] == head_id);
}

#[test]
fn insert_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let document_data = test.document.to_json().unwrap();
  let root_id = &document_data["document"]["root_id"].as_str().unwrap();

  let block_id = inser_text_block(&test.document, root_id, "");
  // insert after block
  let after_block_id = inser_text_block(&test.document, root_id, &block_id);

  let document_data = test.document.to_json().unwrap();
  let document = &document_data["document"];

  let blocks = &document["blocks"];
  let meta = &document["meta"];
  let text_map = &meta["text_map"];
  let children_map = &meta["children_map"];
  let block = blocks[&block_id].as_object().unwrap();
  let after_block = blocks[&after_block_id].as_object().unwrap();
  assert!(block["ty"] == "text");
  assert!(after_block["ty"] == "text");

  let text = block["data"]["text"].as_str().unwrap();
  assert!(text_map[text].is_array());
  let children = block["children"].as_str().unwrap();
  assert!(children_map[children].is_array());
  let parent_id = block["parent"].as_str().unwrap();
  let parent = blocks[parent_id].as_object().unwrap();
  let parent_children_id = parent["children"].as_str().unwrap();
  let parent_children = children_map[parent_children_id].as_array().unwrap();
  assert!(parent_children[0] == block_id);
  assert!(parent_children[1] == after_block_id);
}

#[test]
fn delete_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let document_data = test.document.to_json().unwrap();
  let root_id = &document_data["document"]["root_id"].as_str().unwrap();
  let block_id = inser_text_block(&test.document, root_id, "");
  let parent_id = test.document.get_block(&block_id).unwrap().parent;
  let parent_children_id = test.document.get_block(&parent_id).unwrap().children;
  let data = test.document.get_block(&block_id).unwrap().data;
  let text_id = BlockDataParser::parser(&data).unwrap().text;
  let children_id = test.document.get_block(&block_id).unwrap().children;

  delete_block(&test.document, &block_id);

  assert!(test.document.get_block(&block_id).is_none());

  let document_data = test.document.to_json().unwrap();
  let document = &document_data["document"];
  let blocks = &document["blocks"];
  assert!(blocks[&block_id].is_null());
  let meta = &document["meta"];
  let text_map = &meta["text_map"];
  assert!(text_map[&text_id].is_null());
  let children_map = &meta["children_map"];
  assert!(children_map[&children_id].is_null());
  let parent_children = children_map[parent_children_id].as_array().unwrap();
  assert!(parent_children.len() == 1);
  assert!(parent_children
    .iter()
    .any(|e| e.to_string() != block_id.to_string()));
}
