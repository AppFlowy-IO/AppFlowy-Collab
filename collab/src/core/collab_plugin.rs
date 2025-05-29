use crate::core::awareness::{AwarenessUpdate, Event};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;

use std::sync::Arc;
use tracing::trace;
use yrs::{Doc, TransactionMut};

use crate::core::origin::CollabOrigin;
use crate::error::CollabError;
use crate::preclude::Collab;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CollabPluginType {
  /// The plugin is used for sync data with a remote storage. Only one plugin of this type can be
  /// used per document.
  CloudStorage,
  /// The default plugin type. It can be used for any other purpose.
  Other(String),
}
pub trait CollabPersistence: Send + Sync + 'static {
  fn load_collab_from_disk(&self, collab: &mut Collab) -> Result<(), CollabError>;
}

impl<T> CollabPersistence for Box<T>
where
  T: CollabPersistence,
{
  fn load_collab_from_disk(&self, collab: &mut Collab) -> Result<(), CollabError> {
    (**self).load_collab_from_disk(collab)
  }
}

pub trait CollabPlugin: Send + Sync + 'static {
  /// Called when the plugin is initialized.
  /// The will apply the updates to the current [TransactionMut] which will restore the state of
  /// the document.
  fn init(&self, _object_id: &str, _origin: &CollabOrigin, _doc: &Doc) {}

  /// Called when the plugin is initialized.
  fn did_init(&self, _collab: &Collab, _object_id: &str) {}

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
  fn plugin_type(&self) -> CollabPluginType;

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

  fn did_init(&self, collab: &Collab, _object_id: &str) {
    (**self).did_init(collab, _object_id)
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
pub struct Plugins(pub(crate) Arc<PluginsInner>);
#[derive(Default)]
pub(crate) struct PluginsInner {
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
      head: ArcSwapOption::new(None),
    }));
    for plugin in plugins {
      list.push_front(plugin);
    }
    list
  }

  // Check if a CloudStorage plugin exists in the list
  pub fn has_cloud_plugin(&self) -> bool {
    let mut current = self.0.head.load_full();
    while let Some(node) = current {
      if node.value.plugin_type() == CollabPluginType::CloudStorage {
        return true; // CloudStorage plugin found
      }
      current = node.next.load_full();
    }
    false
  }

  // Remove a plugin based on its type
  pub fn remove_plugin(&self, plugin_type: CollabPluginType) {
    let inner = &*self.0;
    let mut current = inner.head.load_full();
    let mut prev: Option<Arc<Node>> = None;

    while let Some(curr_node) = current {
      if curr_node.value.plugin_type() == plugin_type {
        let next = curr_node.next.load_full();
        match prev {
          Some(prev_node) => {
            prev_node.next.store(next); // Bypass the current node
          },
          None => {
            inner.head.swap(next); // Removing the head node
          },
        }

        trace!("Removed plugin: {:?}", plugin_type);
        curr_node.value.destroy();
        return;
      }

      prev = Some(curr_node.clone());
      current = curr_node.next.load_full();
    }
  }

  // Push a plugin to the front of the list
  pub fn push_front(&self, plugin: Box<dyn CollabPlugin>) -> bool {
    let inner = &*self.0;
    if self.contains_plugin(plugin.plugin_type()) {
      return false;
    }

    let new_node = Arc::new(Node {
      next: ArcSwapOption::new(None),
      value: plugin,
    });

    inner.head.rcu(|old_head| {
      new_node.next.store(old_head.clone());
      Some(new_node.clone())
    });

    true
  }

  pub fn contains_plugin(&self, plugin_type: CollabPluginType) -> bool {
    let mut current = self.0.head.load_full();
    while let Some(node) = current {
      if node.value.plugin_type() == plugin_type {
        return true;
      }
      current = node.next.load_full();
    }
    false
  }

  // Remove all plugins from the list
  pub fn remove_all(&self) -> RemovedPluginsIter {
    let inner = &*self.0;
    let current = inner.head.swap(None);
    RemovedPluginsIter { current }
  }

  // Iterate over each plugin in the list and apply a function to it
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
#[cfg(test)]
mod tests {
  use super::*;

  struct MyPlugin {
    pub plugin_type: CollabPluginType,
  }

  impl CollabPlugin for MyPlugin {
    fn plugin_type(&self) -> CollabPluginType {
      self.plugin_type.clone()
    }

    fn destroy(&self) {
      // In a real implementation, you might want to clean up resources here.
    }
  }

  #[test]
  fn test_push_front_and_contains_plugin() {
    let plugins = Plugins::new(vec![]);

    // Initially, the list should not contain any plugins
    assert!(!plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(!plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Add an Other plugin
    let other_plugin = Box::new(MyPlugin {
      plugin_type: CollabPluginType::Other("PluginA".to_string()),
    });
    assert!(plugins.push_front(other_plugin));

    // The list should now contain the Other plugin
    assert!(plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(!plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Add a CloudStorage plugin
    let cloud_plugin = Box::new(MyPlugin {
      plugin_type: CollabPluginType::CloudStorage,
    });
    assert!(plugins.push_front(cloud_plugin));

    // The list should contain both Other and CloudStorage plugins
    assert!(plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Try to add another CloudStorage plugin (should be rejected)
    let another_cloud_plugin = Box::new(MyPlugin {
      plugin_type: CollabPluginType::CloudStorage,
    });
    assert!(!plugins.push_front(another_cloud_plugin)); // Should return false
  }

  #[test]
  fn test_remove_plugin() {
    let plugins = Plugins::new(vec![
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::Other("PluginA".to_string()),
      }) as Box<dyn CollabPlugin>,
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::CloudStorage,
      }) as Box<dyn CollabPlugin>,
    ]);

    // The list should contain both Other and CloudStorage plugins
    assert!(plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Remove the Other plugin
    plugins.remove_plugin(CollabPluginType::Other("PluginA".to_string()));
    assert!(!plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Remove the CloudStorage plugin
    plugins.remove_plugin(CollabPluginType::CloudStorage);
    assert!(!plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(!plugins.contains_plugin(CollabPluginType::CloudStorage));
  }

  #[test]
  fn test_remove_all() {
    let plugins = Plugins::new(vec![
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::Other("PluginA".to_string()),
      }) as Box<dyn CollabPlugin>,
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::CloudStorage,
      }) as Box<dyn CollabPlugin>,
    ]);

    // The list should contain both Other and CloudStorage plugins
    assert!(plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(plugins.contains_plugin(CollabPluginType::CloudStorage));

    // Remove all plugins
    let mut removed_plugins = plugins.remove_all();

    // Check that the removed plugins iterator contains both plugins
    let removed_plugin_1 = removed_plugins.next().unwrap();
    let removed_plugin_2 = removed_plugins.next().unwrap();
    let types: Vec<_> = vec![
      removed_plugin_1.plugin_type(),
      removed_plugin_2.plugin_type(),
    ];
    assert!(types.contains(&CollabPluginType::Other("PluginA".to_string())));
    assert!(types.contains(&CollabPluginType::CloudStorage));

    // After removing all, the list should be empty
    assert!(!plugins.contains_plugin(CollabPluginType::Other("PluginA".to_string())));
    assert!(!plugins.contains_plugin(CollabPluginType::CloudStorage));
  }

  #[test]
  fn test_each() {
    let plugins = Plugins::new(vec![
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::Other("PluginA".to_string()),
      }) as Box<dyn CollabPlugin>,
      Box::new(MyPlugin {
        plugin_type: CollabPluginType::CloudStorage,
      }) as Box<dyn CollabPlugin>,
    ]);

    // Collect all plugin types using the `each` method
    let mut plugin_types = vec![];
    plugins.each(|plugin| {
      plugin_types.push(plugin.plugin_type());
    });

    // Ensure both Other and CloudStorage plugins were iterated
    assert!(plugin_types.contains(&CollabPluginType::Other("PluginA".to_string())));
    assert!(plugin_types.contains(&CollabPluginType::CloudStorage));
  }
}
