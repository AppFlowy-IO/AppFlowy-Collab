use crate::local_storage::indexeddb::kv_impl::CollabIndexeddb;
use crate::local_storage::kv::keys::{make_doc_id_key, make_doc_state_key, make_state_vector_key};
use std::future::Future;

use async_stream::stream;
use async_trait::async_trait;
use collab::core::awareness::Awareness;

use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_entity::CollabType;
use futures::stream::StreamExt;

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::PersistenceError;
use crate::CollabKVDB;
use anyhow::anyhow;
use collab::core::collab::make_yrs_doc;
use collab::core::collab_plugin::EncodedCollab;
use collab::core::transaction::DocTransactionExtension;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};
use tracing::{error, instrument};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

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

  #[instrument(skip_all)]
  fn flush_doc(&self, db: Arc<CollabIndexeddb>, object_id: &str) {
    let object_id = object_id.to_string();
    tokio::task::spawn_local(|| async move {
      let doc = make_yrs_doc();
      db.load_doc(self.uid, &object_id, doc.clone())
        .await
        .unwrap();

      let encoded_collab = doc.get_encoded_collab_v1();
      db.flush_doc(self.uid, &object_id, &encoded_collab)
        .await
        .unwrap();
    });
  }
}

#[async_trait]
impl CollabPlugin for IndexeddbDiskPlugin {
  async fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    if let Some(db) = self.collab_db.upgrade() {
      let object_id = object_id.to_string();
      let doc = doc.clone();
      let origin = origin.clone();
      let uid = self.uid;

      tokio::task::spawn_local(async move {
        match db.load_doc(uid, &object_id, doc.clone()).await {
          Ok(_) => {},
          Err(err) => {
            if err.is_record_not_found() {
              let mut txn = doc.transact_mut_with(origin);
              let doc_state = txn.encode_diff_v1(&StateVector::default());
              let sv = txn.state_vector().encode_v1();
              drop(txn);

              let f = || async move {
                let doc_id = db.create_doc_id(uid, object_id).await?;
                let doc_state_key = make_doc_state_key(doc_id);
                let sv_key = make_state_vector_key(doc_id);
                db.set_data(doc_state_key, doc_state).await?;
                db.set_data(sv_key, sv).await?;
                Ok::<(), PersistenceError>(())
              };
              if let Err(err) = f().await {
                error!("failed to create doc_id: {:?}", err);
              }
            } else {
              error!("failed to get encoded collab: {:?}", err);
            }
          },
        }
      });
    } else {
      tracing::warn!("collab_db is dropped");
    }
  }
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _last_sync_at: i64) {
    self.did_load.store(true, SeqCst);
  }

  fn flush(&self, object_id: &str, doc: &Doc) {
    if let Some(db) = self.collab_db.upgrade() {
      self.flush_doc(db, object_id);
    }
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
    stream.for_each(|_data| async {}).await;
  }
}
