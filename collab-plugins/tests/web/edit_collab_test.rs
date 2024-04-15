use assert_json_diff::assert_json_eq;
use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
use collab_entity::CollabType;
use collab_plugins::local_storage::indexeddb::CollabIndexeddb;
use collab_plugins::local_storage::indexeddb::IndexeddbDiskPlugin;
use js_sys::Promise;
use serde_json::json;
use std::sync::{Arc, Once};
use tokio::task::LocalSet;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;
use web_sys::window;

#[wasm_bindgen_test]
async fn edit_collab_with_indexeddb_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      setup_log();
      let object_id = Uuid::new_v4().to_string();
      let uid: i64 = 1;
      let db = Arc::new(CollabIndexeddb::new().await.unwrap());
      let collab = create_collab(uid, object_id.clone(), &db).await;
      collab.lock().insert("message", "hello world");
      let json_1 = collab.lock().to_json_value();
      drop(collab);

      // sleep 2 secs to wait for the disk plugin to flush the data
      sleep(2000).await;
      let collab_from_disk = create_collab(uid, object_id.clone(), &db).await;
      let json_2 = collab_from_disk.lock().to_json_value();
      assert_json_eq!(
        json_2,
        json!({
          "message": "hello world"
        })
      );
      assert_json_eq!(json_1, json_2);
    })
    .await;
}

#[wasm_bindgen_test]
async fn flush_collab_with_indexeddb_test() {
  let local = LocalSet::new();
  local
    .run_until(async {
      setup_log();
      let object_id = Uuid::new_v4().to_string();
      let uid: i64 = 1;
      let db = Arc::new(CollabIndexeddb::new().await.unwrap());
      let collab = create_collab(uid, object_id.clone(), &db).await;
      collab.lock().insert("1", "a");
      sleep(100).await;
      collab.lock().insert("2", "b");
      sleep(100).await;
      collab.lock().insert("3", "c");
      sleep(100).await;
      let json_1 = collab.lock().to_json_value();
      collab.lock().flush();

      // sleep 2 secs to wait for the disk plugin to flush the data
      sleep(2000).await;

      // after flush, all the updates will be removed. Only the final doc state will be saved to disk
      let updates = db.get_all_updates(uid, &object_id).await.unwrap();
      assert_eq!(updates.len(), 0);

      let collab_from_disk = create_collab(uid, object_id.clone(), &db).await;
      let json_2 = collab_from_disk.lock().to_json_value();
      assert_json_eq!(json_1, json_2);
    })
    .await;
}

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    tracing_wasm::set_as_global_default();
  });
}

pub async fn create_collab(
  uid: i64,
  doc_id: String,
  db: &Arc<CollabIndexeddb>,
) -> Arc<MutexCollab> {
  let collab = Arc::new(
    CollabBuilder::new(uid, &doc_id)
      .with_device_id("1")
      .build()
      .unwrap(),
  );
  let disk_plugin = IndexeddbDiskPlugin::new(uid, doc_id, CollabType::Document, Arc::downgrade(db));
  collab.lock().add_plugin(Box::new(disk_plugin));
  collab.lock().initialize();
  sleep(1000).await;
  collab
}

async fn sleep(ms: i32) {
  let promise = Promise::new(&mut |resolve, _| {
    let closure = Closure::once_into_js(move || {
      resolve.call0(&JsValue::NULL).unwrap();
    });

    window()
      .unwrap()
      .set_timeout_with_callback_and_timeout_and_arguments_0(closure.as_ref().unchecked_ref(), ms)
      .unwrap();
  });

  JsFuture::from(promise).await.unwrap();
}
