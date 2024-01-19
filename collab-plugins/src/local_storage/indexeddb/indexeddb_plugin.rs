use crate::local_storage::indexeddb::kv_impl::CollabIndexeddb;
use crate::local_storage::kv::{get_id_for_key, KVStore, PersistenceError};
use async_stream::stream;
use async_trait::async_trait;
use collab::core::awareness::Awareness;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_entity::CollabType;
use futures::stream::StreamExt;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};
use tracing::error;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

pub struct IndexeddbDiskPlugin {
  uid: i64,
  object_id: String,
  collab_type: CollabType,
  collab_db: Weak<CollabIndexeddb>,
  did_load: Arc<AtomicBool>,
}

impl IndexeddbDiskPlugin {
  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabIndexeddb>,
  ) -> Self {
    let did_load = Arc::new(AtomicBool::new(false));
    Self {
      uid,
      object_id,
      collab_type,
      did_load,
      collab_db,
    }
  }
}

#[async_trait]
impl CollabPlugin for IndexeddbDiskPlugin {
  async fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    if let Some(db) = self.collab_db.upgrade() {
      let object_id = object_id.to_string();
      let (tx, rx) = tokio::sync::oneshot::channel();
      tokio::task::spawn_local(async move {
        let encoded_collab = db.get_encoded_collab(&object_id).await;
        let _ = tx.send(encoded_collab);
      });

      match rx.await {
        Ok(Ok(encoded_collab)) => {
          let mut txn = doc.transact_mut_with(origin.clone());
          if let Ok(update) = Update::decode_v1(&encoded_collab.doc_state) {
            txn.apply_update(update);
            txn.commit();
            drop(txn);
          } else {
            error!("failed to decode update");
          }
        },
        Ok(Err(err)) => {
          if !err.is_record_not_found() {
            error!("failed to get encoded collab: {:?}", err);
          }
        },
        Err(err) => {
          error!("receiver error: {:?}", err);
        },
      }
    } else {
      tracing::warn!("collab_db is dropped");
    }
  }
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _last_sync_at: i64) {
    self.did_load.store(true, SeqCst);
  }
}

type DocUpdateStreamReceiver = tokio::sync::mpsc::Receiver<DocUpdate>;
struct DocUpdateStream {
  collab_db: Weak<CollabIndexeddb>,
  receiver: Option<DocUpdateStreamReceiver>,
}

#[derive(Clone)]
enum DocUpdate {
  Update(Vec<u8>),
}

impl DocUpdateStream {
  fn new(collab_db: Weak<CollabIndexeddb>, receiver: DocUpdateStreamReceiver) -> Self {
    Self {
      collab_db,
      receiver: Some(receiver),
    }
  }

  async fn run(mut self) {
    let mut receiver = self.receiver.take().expect("Only take once");
    let stream = stream! {
        loop {
            match receiver.recv().await {
                Some(data) => yield data,
                None => break,
            }
        }
    };
    stream.for_each(|data| async {}).await;
  }
}
