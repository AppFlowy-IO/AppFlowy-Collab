use std::sync::Arc;

use y_sync::awareness::Awareness;
use yrs::{Transaction, TransactionMut};

use crate::core::origin::CollabOrigin;

#[derive(Debug, Eq, PartialEq)]
pub enum CollabPluginType {
  /// The plugin is used for sync data with a remote storage. Only one plugin of this type can be
  /// used per document.
  CloudStorage,
  /// The default plugin type. It can be used for any other purpose.
  Other,
}

pub trait CollabPlugin: Send + Sync + 'static {
  /// Called when the plugin is initialized.
  /// The will apply the updates to the current [TransactionMut] which will restore the state of
  /// the document.
  fn init(&self, _object_id: &str, _txn: &mut TransactionMut) {}

  /// Called when the plugin is initialized.
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {}

  /// Called when the plugin receives an update. It happens after the [TransactionMut] commit to
  /// the Yrs document.
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, _update: &[u8]) {}

  /// Called when the plugin receives a local update
  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, _update: &[u8]) {}

  /// Called after each [TransactionMut]
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {}

  /// Returns the type of the plugin.
  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other
  }
}

/// Implement the [CollabPlugin] trait for Box<T> and Arc<T> where T implements CollabPlugin.
impl<T> CollabPlugin for Box<T>
where
  T: CollabPlugin,
{
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    (**self).init(object_id, txn)
  }

  fn did_init(&self, awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    (**self).did_init(awareness, _object_id, _txn)
  }

  fn receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).receive_update(object_id, txn, update)
  }
}

impl<T> CollabPlugin for Arc<T>
where
  T: CollabPlugin,
{
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    (**self).init(object_id, txn)
  }

  fn did_init(&self, awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    (**self).did_init(awareness, _object_id, _txn)
  }

  fn receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).receive_update(object_id, txn, update)
  }
}
