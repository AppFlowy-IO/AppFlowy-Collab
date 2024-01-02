use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_repr::*;
use yrs::{Doc, TransactionMut};

use crate::core::awareness::Awareness;
use crate::core::origin::CollabOrigin;

#[derive(Debug, Eq, PartialEq)]
pub enum CollabPluginType {
  /// The plugin is used for sync data with a remote storage. Only one plugin of this type can be
  /// used per document.
  CloudStorage,
  /// The default plugin type. It can be used for any other purpose.
  Other,
}

#[async_trait]
pub trait CollabPlugin: Send + Sync + 'static {
  /// Called when the plugin is initialized.
  /// The will apply the updates to the current [TransactionMut] which will restore the state of
  /// the document.
  #[cfg(not(feature = "async-plugin"))]
  fn init(&self, _object_id: &str, _origin: &CollabOrigin, _doc: &Doc) {}

  #[cfg(feature = "async-plugin")]
  async fn init(&self, _object_id: &str, _origin: &CollabOrigin, _doc: &Doc) {}

  /// Called when the plugin is initialized.
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _last_sync_at: i64) {}

  /// Called when the plugin receives an update. It happens after the [TransactionMut] commit to
  /// the Yrs document.
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, _update: &[u8]) {}

  /// Called when the plugin receives a local update.
  /// We use the [CollabOrigin] to know if the update comes from the local user or from a remote
  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, _update: &[u8]) {}

  /// Called after each [TransactionMut]
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {}

  /// Returns the type of the plugin.
  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other
  }

  /// Notifies the plugin that the collab object has been reset. It happens when the collab object
  /// is ready to sync from the remote. When reset is called, the plugin should reset its state.
  fn reset(&self, _object_id: &str) {}

  /// Flush the data to the storage. It will remove all existing updates and insert the state vector
  /// and doc_state.
  fn flush(&self, _object_id: &str, _doc: &Doc) {}
}

/// Implement the [CollabPlugin] trait for Box<T> and Arc<T> where T implements CollabPlugin.
#[async_trait]
impl<T> CollabPlugin for Box<T>
where
  T: CollabPlugin,
{
  #[cfg(not(feature = "async-plugin"))]
  fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    (**self).init(object_id, origin, doc);
  }

  #[cfg(feature = "async-plugin")]
  async fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    (**self).init(object_id, origin, doc).await;
  }

  fn did_init(&self, _awareness: &Awareness, _object_id: &str, last_sync_at: i64) {
    (**self).did_init(_awareness, _object_id, last_sync_at)
  }

  fn receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).receive_update(object_id, txn, update)
  }
}

#[async_trait]
impl<T> CollabPlugin for Arc<T>
where
  T: CollabPlugin,
{
  #[cfg(not(feature = "async-plugin"))]
  fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    (**self).init(object_id, origin, doc);
  }

  #[cfg(feature = "async-plugin")]
  async fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    (**self).init(object_id, origin, doc).await;
  }

  fn did_init(&self, _awareness: &Awareness, _object_id: &str, last_sync_at: i64) {
    (**self).did_init(_awareness, _object_id, last_sync_at)
  }

  fn receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).receive_update(object_id, txn, update)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct EncodedCollab {
  pub state_vector: Bytes,
  pub doc_state: Bytes,
  #[serde(default)]
  pub version: EncoderVersion,
}

#[derive(Default, Serialize_repr, Deserialize_repr, Eq, PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum EncoderVersion {
  #[default]
  V1 = 0,
  V2 = 1,
}

impl EncodedCollab {
  pub fn new_v1<T: Into<Bytes>>(state_vector: T, doc_state: T) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V1,
    }
  }

  pub fn new_v2<T: Into<Bytes>>(state_vector: T, doc_state: T) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V2,
    }
  }

  pub fn encode_to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(self)
  }

  pub fn decode_from_bytes(encoded: &[u8]) -> Result<EncodedCollab, bincode::Error> {
    bincode::deserialize(encoded)
  }
}
