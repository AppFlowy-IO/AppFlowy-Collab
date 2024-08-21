use crate::core::awareness::{AwarenessUpdate, Event};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use yrs::{Doc, TransactionMut};

use crate::core::origin::CollabOrigin;
use crate::preclude::Collab;

#[derive(Debug, Eq, PartialEq)]
pub enum CollabPluginType {
  /// The plugin is used for sync data with a remote storage. Only one plugin of this type can be
  /// used per document.
  CloudStorage,
  /// The default plugin type. It can be used for any other purpose.
  Other,
}
pub trait CollabPersistence: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);
}

impl<T> CollabPersistence for Box<T>
where
  T: CollabPersistence,
{
  fn load_collab(&self, collab: &mut Collab) {
    (**self).load_collab(collab);
  }
}

pub trait CollabPlugin: Send + Sync + 'static {
  /// Called when the plugin is initialized.
  /// The will apply the updates to the current [TransactionMut] which will restore the state of
  /// the document.
  fn init(&self, _object_id: &str, _origin: &CollabOrigin, _doc: &Doc) {}

  /// Called when the plugin is initialized.
  fn did_init(&self, _collab: &Collab, _object_id: &str, _last_sync_at: i64) {}

  /// Called when the plugin receives an update. It happens after the [TransactionMut] commit to
  /// the Yrs document.
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, _update: &[u8]) {}

  /// Called when the plugin receives a local update.
  /// We use the [CollabOrigin] to know if the update comes from the local user or from a remote
  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, _update: &[u8]) {}

  fn receive_local_state(
    &self,
    _origin: &CollabOrigin,
    _object_id: &str,
    _event: &Event,
    _update: &AwarenessUpdate,
  ) {
  }

  /// Called after each [TransactionMut]
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {}

  /// Returns the type of the plugin.
  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other
  }

  /// Flush the data to the storage. It will remove all existing updates and insert the state vector
  /// and doc_state.

  fn start_init_sync(&self) {}

  /// Called when the plugin is removed
  fn destroy(&self) {}
}

/// Implement the [CollabPlugin] trait for Box<T> and Arc<T> where T implements CollabPlugin.
///
/// A limitation of manually implementing traits for Arc<T> is that any default methods in the trait
/// must also be explicitly implemented for Arc<T>. If not, Arc<T> will default to using the trait's
/// default method implementations, even if the underlying type T has its own specific implementations
#[async_trait]
impl<T> CollabPlugin for Box<T>
where
  T: CollabPlugin,
{
  fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    (**self).init(object_id, origin, doc);
  }

  fn did_init(&self, collab: &Collab, _object_id: &str, last_sync_at: i64) {
    (**self).did_init(collab, _object_id, last_sync_at)
  }

  fn receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).receive_update(object_id, txn, update)
  }

  fn receive_local_update(&self, origin: &CollabOrigin, object_id: &str, update: &[u8]) {
    (**self).receive_local_update(origin, object_id, update)
  }
  fn receive_local_state(
    &self,
    origin: &CollabOrigin,
    object_id: &str,
    event: &Event,
    update: &AwarenessUpdate,
  ) {
    (**self).receive_local_state(origin, object_id, event, update)
  }

  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    (**self).after_transaction(object_id, txn)
  }
  fn plugin_type(&self) -> CollabPluginType {
    (**self).plugin_type()
  }

  fn start_init_sync(&self) {
    (**self).start_init_sync()
  }

  fn destroy(&self) {
    (**self).destroy()
  }
}

#[derive(Clone, Default)]
pub struct Plugins(Arc<PluginsInner>);

#[derive(Default)]
struct PluginsInner {
  has_cloud_storage: AtomicBool,
  head: ArcSwapOption<Node>,
}

struct Node {
  next: ArcSwapOption<Node>,
  value: Box<dyn CollabPlugin>,
}

impl Plugins {
  pub fn new<I>(plugins: I) -> Self
  where
    I: IntoIterator<Item = Box<dyn CollabPlugin>>,
  {
    let list = Plugins(Arc::new(PluginsInner {
      has_cloud_storage: AtomicBool::new(false),
      head: ArcSwapOption::new(None),
    }));
    for plugin in plugins {
      list.push_front(plugin);
    }
    list
  }

  pub fn push_front(&self, plugin: Box<dyn CollabPlugin>) -> bool {
    let inner = &*self.0;
    if plugin.plugin_type() == CollabPluginType::CloudStorage {
      let already_existed = inner
        .has_cloud_storage
        .swap(true, std::sync::atomic::Ordering::SeqCst);
      if already_existed {
        return false; // skip adding the plugin
      }
    }
    let new = Arc::new(Node {
      next: ArcSwapOption::new(None),
      value: plugin,
    });
    inner.head.rcu(|old_head| {
      new.next.store(old_head.clone());
      Some(new.clone())
    });
    true
  }

  pub fn remove_all(&self) -> RemovedPluginsIter {
    let inner = &*self.0;
    let current = inner.head.swap(None);
    inner
      .has_cloud_storage
      .store(false, std::sync::atomic::Ordering::SeqCst);
    RemovedPluginsIter { current }
  }

  pub fn each<F>(&self, mut f: F)
  where
    F: FnMut(&Box<dyn CollabPlugin>),
  {
    let mut curr = self.0.head.load_full();
    while let Some(node) = curr {
      f(&node.value);
      curr = node.next.load_full();
    }
  }
}

pub struct RemovedPluginsIter {
  current: Option<Arc<Node>>,
}

impl Iterator for RemovedPluginsIter {
  type Item = Box<dyn CollabPlugin>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.current.take() {
      None => None,
      Some(node) => {
        self.current = node.next.load_full();
        let node = Arc::into_inner(node)?;
        Some(node.value)
      },
    }
  }
}
