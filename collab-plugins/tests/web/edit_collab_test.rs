use collab::core::collab_plugin::EncodedCollab;
use collab_plugins::local_storage::indexeddb::kv_impl::CollabIndexeddb;
use uuid::Uuid;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
async fn indexeddb_put_and_get_encoded_collab_test() {
  let db = CollabIndexeddb::new().await.unwrap();
  let object_id = Uuid::new_v4().to_string();
  let uid: i64 = 1;
  let encoded_collab = EncodedCollab {
    state_vector: vec![1, 2, 3].into(),
    doc_state: vec![4, 5, 6].into(),
    version: collab::core::collab_plugin::EncoderVersion::V1,
  };

  db.create_doc(uid, &object_id, &encoded_collab)
    .await
    .unwrap();
  let encoded_collab_from_db = db.get_encoded_collab(uid, &object_id).await.unwrap();

  assert_eq!(
    encoded_collab.state_vector,
    encoded_collab_from_db.state_vector
  );
  assert_eq!(encoded_collab.doc_state, encoded_collab_from_db.doc_state);
}
