use crate::local_storage::indexeddb::kv_impl::CollabIndexeddb;
use crate::local_storage::kv::keys::{make_doc_state_key, make_state_vector_key};

use async_stream::stream;
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabType;
use futures::stream::StreamExt;

use crate::local_storage::kv::PersistenceError;
use collab::core::collab::make_yrs_doc;
use collab::core::transaction::DocTransactionExtension;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};
use tracing::{error, instrument};
use yrs::{Doc, TransactionMut};

pub struct IndexeddbDiskPlugin {
  uid: i64,
  #[allow(dead_code)]
  object_id: String,
  #[allow(dead_code)]
  collab_type: CollabType,
  collab_db: Weak<CollabIndexeddb>,
  did_load: Arc<AtomicBool>,
  edit_sender: DocEditStreamSender,
}

impl IndexeddbDiskPlugin {
  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabIndexeddb>,
  ) -> Self {
    let did_load = Arc::new(AtomicBool::new(false));
    let (edit_sender, rx) = tokio::sync::mpsc::unbounded_channel();
    let edit_stream = DocEditStream::new(uid, &object_id, collab_db.clone(), rx);
    tokio::task::spawn_local(edit_stream.run());
    Self {
      uid,
      object_id,
      collab_type,
      did_load,
      collab_db,
      edit_sender,
    }
  }

  #[instrument(skip_all)]
  fn flush_doc(&self, db: Arc<CollabIndexeddb>, object_id: &str) {
    let uid = self.uid;
    let object_id = object_id.to_string();
    tokio::task::spawn_local(async move {
      let doc = make_yrs_doc(false);
      db.load_doc(uid, &object_id, doc.clone()).await.unwrap();
      let encoded_collab = doc.get_encoded_collab_v1();
      db.flush_doc(uid, &object_id, &encoded_collab)
        .await
        .unwrap();
    });
  }
}

impl CollabPlugin for IndexeddbDiskPlugin {
  fn init(&self, object_id: &str, _origin: &CollabOrigin, doc: &Doc) {
    if let Some(db) = self.collab_db.upgrade() {
      let object_id = object_id.to_string();
      let doc = doc.clone();
      let uid = self.uid;

      tokio::task::spawn_local(async move {
        match db.load_doc(uid, &object_id, doc.clone()).await {
          Ok(_) => {},
          Err(err) => {
            if err.is_record_not_found() {
              let encoded_collab = doc.get_encoded_collab_v1();
              let f = || async move {
                let doc_id = db.create_doc_id(uid, object_id).await?;
                let doc_state_key = make_doc_state_key(doc_id);
                let sv_key = make_state_vector_key(doc_id);
                db.set_data(doc_state_key, encoded_collab.doc_state).await?;
                db.set_data(sv_key, encoded_collab.state_vector).await?;
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

  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_load.load(SeqCst) {
      return;
    }
    if let Err(err) = self.edit_sender.send(DocUpdate::Update(update.to_vec())) {
      error!("failed to send update: {}", err);
    }
  }

  fn did_init(&self, _collab: &Collab, _object_id: &str, _last_sync_at: i64) {
    self.did_load.store(true, SeqCst);
  }

  fn flush(&self, object_id: &str, _doc: &Doc) {
    if let Some(db) = self.collab_db.upgrade() {
      self.flush_doc(db, object_id);
    }
  }
}

type DocEditStreamSender = tokio::sync::mpsc::UnboundedSender<DocUpdate>;
type DocEditStreamReceiver = tokio::sync::mpsc::UnboundedReceiver<DocUpdate>;
struct DocEditStream {
  uid: i64,
  object_id: String,
  collab_db: Weak<CollabIndexeddb>,
  receiver: Option<DocEditStreamReceiver>,
}

#[derive(Clone)]
enum DocUpdate {
  Update(Vec<u8>),
}

impl DocEditStream {
  fn new(
    uid: i64,
    object_id: &str,
    collab_db: Weak<CollabIndexeddb>,
    receiver: DocEditStreamReceiver,
  ) -> Self {
    Self {
      uid,
      object_id: object_id.to_string(),
      collab_db,
      receiver: Some(receiver),
    }
  }

  async fn run(mut self) {
    let mut receiver = self.receiver.take().expect("Only take once");
    while let Some(data) = receiver.recv().await {
      match data {
        DocUpdate::Update(update) => {
          if let Some(db) = self.collab_db.upgrade() {
            if let Err(err) = db.push_update(self.uid, &self.object_id, &update).await {
              error!("failed to push update: {}", err);
            }
          }
        },
      }
    }
  }
}
