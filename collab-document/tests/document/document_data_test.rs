use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_document::document::Document;
use collab_document::document_data::default_document_data;

#[test]
fn get_default_data_test() {
  let document_id = "1";
  let data = default_document_data(document_id);
  assert!(!data.page_id.is_empty());
  assert!(!data.blocks.is_empty());
  assert!(!data.meta.children_map.is_empty());
  assert!(data.meta.text_map.is_some());
  assert!(data.meta.text_map.is_some());
  assert_eq!(data.meta.text_map.unwrap().len(), 1);

  let document_id = "2";
  let data = default_document_data(document_id);
  println!("{:?}", data);
  assert!(!data.page_id.is_empty());
  assert_eq!(data.blocks.len(), 2);
  assert_eq!(data.meta.children_map.len(), 2);
  assert!(data.meta.text_map.is_some());
  assert_eq!(data.meta.text_map.unwrap().len(), 1);
}

#[test]
fn validate_document_data() {
  let document_id = "1";
  let document_data = default_document_data(document_id);
  let document = Document::create(document_id, document_data, default_client_id()).unwrap();
  assert!(document.validate().is_ok());

  let options = CollabOptions::new(document_id.to_string(), default_client_id());
  let new_collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let result = Document::open(new_collab);
  assert!(result.is_err())
}
