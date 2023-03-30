use crate::util::create_document;

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
