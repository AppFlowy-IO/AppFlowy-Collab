use collab::core::collab_plugin::EncodedCollab;
use collab_plugins::local_storage::indexeddb::kv_impl::CollabIndexeddb;

use uuid::Uuid;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
async fn indexeddb_put_and_get_encoded_collab_test() {
  let db = CollabIndexeddb::new().await.unwrap();
  let encoded_collab = EncodedCollab {
    state_vector: vec![1, 2, 3].into(),
    doc_state: vec![4, 5, 6].into(),
    version: collab::core::collab_plugin::EncoderVersion::V1,
  };

  db.save_encoded_collab("test", &encoded_collab)
    .await
    .unwrap();
  let encoded_collab_from_db = db.get_encoded_collab("test").await.unwrap();

  assert_eq!(
    encoded_collab.state_vector,
    encoded_collab_from_db.state_vector
  );
  assert_eq!(encoded_collab.doc_state, encoded_collab_from_db.doc_state);
}

#[wasm_bindgen_test]
async fn indexeddb_get_non_exist_encoded_collab_test() {
  let db = CollabIndexeddb::new().await.unwrap();
  let object_id = Uuid::new_v4().to_string();
  let error = db.get_encoded_collab(&object_id).await.unwrap_err();
  assert_eq!(
    error.to_string(),
    format!("object with given id: {} is not found", object_id)
  );
}
