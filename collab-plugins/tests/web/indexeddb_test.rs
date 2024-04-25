use collab::entity::EncodedCollab;
use collab_plugins::local_storage::indexeddb::CollabIndexeddb;
use tokio::task::LocalSet;
use uuid::Uuid;
use wasm_bindgen_test::*;
use yrs::Doc;

#[wasm_bindgen_test]
async fn indexeddb_put_and_get_encoded_collab_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      let db = CollabIndexeddb::new().await.unwrap();
      let object_id = Uuid::new_v4().to_string();
      let uid: i64 = 1;
      let encoded_collab = EncodedCollab {
        state_vector: vec![1, 2, 3].into(),
        doc_state: vec![4, 5, 6].into(),
        version: collab::entity::EncoderVersion::V1,
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
    })
    .await;
}

#[wasm_bindgen_test]
async fn indexeddb_get_non_exist_encoded_collab_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      let db = CollabIndexeddb::new().await.unwrap();
      let object_id = Uuid::new_v4().to_string();
      let doc = Doc::new();
      let uid: i64 = 1;
      let error = db.load_doc(uid, &object_id, doc).await.unwrap_err();
      assert!(error.is_record_not_found());
    })
    .await;
}

#[wasm_bindgen_test]
async fn indexeddb_push_update_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      let db = CollabIndexeddb::new().await.unwrap();
      let object_id = Uuid::new_v4().to_string();
      let uid: i64 = 1;

      db.create_doc_id(uid, &object_id).await.unwrap();
      let update_1 = vec![1, 2, 3];
      db.push_update(uid, &object_id, &update_1).await.unwrap();

      let update_2 = vec![4, 5, 6];
      db.push_update(uid, &object_id, &update_2).await.unwrap();

      let update_3 = vec![7, 8, 9];
      db.push_update(uid, &object_id, &update_3).await.unwrap();

      let update_4 = vec![10, 11, 12];
      db.push_update(uid, &object_id, &update_4).await.unwrap();

      let updates = db.get_all_updates(uid, &object_id).await.unwrap();
      assert_eq!(updates.len(), 4);
      assert_eq!(updates[0], update_1);
      assert_eq!(updates[1], update_2);
      assert_eq!(updates[2], update_3);
      assert_eq!(updates[3], update_4);
    })
    .await;
}

#[wasm_bindgen_test]
async fn indexeddb_flush_doc_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      let db = CollabIndexeddb::new().await.unwrap();
      let object_id = Uuid::new_v4().to_string();
      let uid: i64 = 1;

      db.create_doc_id(uid, &object_id).await.unwrap();
      let update_1 = vec![1, 2, 3];
      db.push_update(uid, &object_id, &update_1).await.unwrap();

      let update_2 = vec![4, 5, 6];
      db.push_update(uid, &object_id, &update_2).await.unwrap();

      let update_3 = vec![7, 8, 9];
      db.push_update(uid, &object_id, &update_3).await.unwrap();

      let update_4 = vec![10, 11, 12];
      db.push_update(uid, &object_id, &update_4).await.unwrap();

      let encoded_collab = EncodedCollab {
        state_vector: vec![1, 2, 3].into(),
        doc_state: vec![4, 5, 6].into(),
        version: collab::entity::EncoderVersion::V1,
      };
      db.flush_doc(uid, &object_id, &encoded_collab)
        .await
        .unwrap();

      let updates = db.get_all_updates(uid, &object_id).await.unwrap();
      assert_eq!(updates.len(), 0);
    })
    .await;
}
