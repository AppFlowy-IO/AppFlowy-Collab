use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::vec::IntoIter;

use parking_lot::{Mutex, RwLock};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use y_sync::awareness::Awareness;
use yrs::block::Prelim;
use yrs::types::map::MapEvent;
use yrs::types::{ToJson, Value};
use yrs::{
  ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Observable, Options, Origin, ReadTxn,
  Subscription, Transact, Transaction, TransactionMut, Update, UpdateSubscription,
};

use crate::core::collab_plugin::CollabPlugin;
use crate::core::map_wrapper::{CustomMapRef, MapRefWrapper};
use crate::preclude::{ArrayRefWrapper, JsonValue};
use crate::util::insert_json_value_to_map_ref;

pub const DATA_SECTION: &str = "data";

type AfterTransactionSubscription = Subscription<Arc<dyn Fn(&mut TransactionMut)>>;

pub type MapSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &MapEvent)>;
pub type MapSubscription = Subscription<MapSubscriptionCallback>;

/// A [Collab] is a wrapper around a [Doc] and [Awareness] that provides a set
/// of helper methods for interacting with the [Doc] and [Awareness]. The [MutexCollab]
/// is a thread-safe wrapper around the [Collab].
pub struct Collab {
  /// The object id can be the document id or the database id. It must be unique for
  /// each [Collab] instance.
  pub object_id: String,

  /// This [ClientCollabOrigin] is used to verify the origin of a [Transaction] when
  /// applying a remote update.
  origin: ClientCollabOrigin,

  /// The [Doc] is the main data structure that is used to store the data.
  doc: Doc,
  /// The [Awareness] is used to track the awareness of the other peers.
  awareness: Awareness,

  /// Every [Collab] instance has a data section that can be used to store
  data: MapRef,

  /// A list of plugins that are used to extend the functionality of the [Collab].
  plugins: Plugins,

  update_subscription: RwLock<Option<UpdateSubscription>>,
  after_txn_subscription: RwLock<Option<AfterTransactionSubscription>>,
}

impl Collab {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T, plugins: Vec<Arc<dyn CollabPlugin>>) -> Collab {
    let origin = ClientCollabOrigin::new(uid, "");
    Self::new_with_origin(origin, object_id, plugins)
  }

  pub fn new_with_origin<T: AsRef<str>>(
    origin: ClientCollabOrigin,
    object_id: T,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Collab {
    let object_id = object_id.as_ref().to_string();
    let doc = Doc::with_options(Options {
      skip_gc: true,
      ..Options::default()
    });
    let data = doc.get_or_insert_map(DATA_SECTION);
    let plugins = Plugins::new(plugins);
    let awareness = Awareness::new(doc.clone());
    Self {
      origin,
      object_id,
      doc,
      awareness,
      data,
      plugins,
      update_subscription: Default::default(),
      after_txn_subscription: Default::default(),
    }
  }

  pub fn with_device_id(mut self, device_id: String) -> Self {
    self.origin.device_id = Some(device_id);
    self
  }

  /// Returns the [Doc] associated with the [Collab].
  pub fn get_doc(&self) -> &Doc {
    &self.doc
  }

  /// Returns the [Awareness] associated with the [Collab].
  pub fn get_awareness(&self) -> &Awareness {
    &self.awareness
  }

  pub fn get_mut_awareness(&mut self) -> &mut Awareness {
    &mut self.awareness
  }

  /// Add a plugin to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugin(&mut self, plugin: Arc<dyn CollabPlugin>) {
    self.plugins.write().push(plugin);
  }

  /// Add plugins to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugins(&mut self, plugins: Vec<Arc<dyn CollabPlugin>>) {
    let mut write_guard = self.plugins.write();
    for plugin in plugins {
      write_guard.push(plugin);
    }
  }

  /// When calling this method, the [Collab]'s doc will be initialized with the plugins. The plugin's
  /// callbacks will be called in the order they are added..
  ///
  /// This method should be called after all plugins are added.
  pub fn initial(&self) {
    {
      let mut txn = self.transact_mut();
      self
        .plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.init(&self.object_id, &mut txn));
      drop(txn);
    }

    let (update_subscription, after_txn_subscription) = observe_doc(
      &self.doc,
      self.object_id.clone(),
      self.plugins.clone(),
      self.origin.clone(),
    );

    *self.update_subscription.write() = Some(update_subscription);
    *self.after_txn_subscription.write() = Some(after_txn_subscription);

    {
      let txn = self.doc.transact();
      self
        .plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.did_init(&self.awareness, &self.object_id, &txn));
    }
  }

  pub fn observer_attrs<F>(&mut self, f: F) -> MapSubscription
  where
    F: Fn(&TransactionMut, &MapEvent) + 'static,
  {
    self.data.observe(f)
  }

  pub fn get(&self, key: &str) -> Option<Value> {
    let txn = self.doc.transact();
    self.data.get(&txn, key)
  }

  pub fn get_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<Value> {
    self.data.get(txn, key)
  }

  pub fn insert<V: Prelim>(&self, key: &str, value: V) -> V::Return {
    self.with_transact_mut(|txn| self.insert_with_txn(txn, key, value))
  }

  pub fn insert_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    value: V,
  ) -> V::Return {
    self.data.insert(txn, key, value)
  }

  pub fn insert_json_with_path<T: Serialize>(&mut self, path: Vec<String>, key: &str, value: T) {
    let mut map = if path.is_empty() {
      None
    } else {
      let txn = self.transact();
      self.get_map_with_txn(&txn, path).map(|m| m.into_inner())
    };

    self.with_transact_mut(|txn| {
      if map.is_none() {
        map = Some(
          self
            .data
            .insert(txn, key, MapPrelim::<lib0::any::Any>::new()),
        );
      }
      let value = serde_json::to_value(&value).unwrap();
      insert_json_value_to_map_ref(key, &value, map.unwrap(), txn);
    });
  }

  pub fn create_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRefWrapper {
    let map = MapPrelim::<lib0::any::Any>::new();
    let map_ref = self.data.insert(txn, key, map);
    self.map_wrapper_with(map_ref)
  }

  pub fn get_json_with_path<T: DeserializeOwned>(&self, path: impl Into<Path>) -> Option<T> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let txn = self.transact();
    let map = self.get_map_with_txn(&txn, path)?;
    drop(txn);

    let json_str = map.to_json();
    let object = serde_json::from_str::<T>(&json_str).ok()?;
    Some(object)
  }

  pub fn get_map_with_path<M: CustomMapRef>(&self, path: impl Into<Path>) -> Option<M> {
    let txn = self.doc.transact();
    let map_ref = self.get_map_with_txn(&txn, path)?;
    Some(M::from_map_ref(map_ref))
  }

  pub fn get_map_with_txn<P: Into<Path>, T: ReadTxn>(
    &self,
    txn: &T,
    path: P,
  ) -> Option<MapRefWrapper> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let mut iter = path.into_iter();
    let mut map_ref = self.data.get(txn, &iter.next().unwrap())?.to_ymap();
    for path in iter {
      map_ref = map_ref?.get(txn, &path)?.to_ymap();
    }
    map_ref.map(|map_ref| self.map_wrapper_with(map_ref))
  }

  pub fn get_array_with_txn<P: Into<Path>, T: ReadTxn>(
    &self,
    txn: &T,
    path: P,
  ) -> Option<ArrayRefWrapper> {
    let path = path.into();
    let array_ref = self
      .get_ref_from_path_with_txn(txn, path)
      .map(|value| value.to_yarray())?;

    array_ref.map(|array_ref| self.array_wrapper_with(array_ref))
  }

  pub fn create_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    values: Vec<V>,
  ) -> ArrayRefWrapper {
    let array_ref = self.data.insert(txn, key, ArrayPrelim::from(values));
    self.array_wrapper_with(array_ref)
  }

  fn get_ref_from_path_with_txn<T: ReadTxn>(&self, txn: &T, mut path: Path) -> Option<Value> {
    if path.is_empty() {
      return None;
    }

    if path.len() == 1 {
      return self.data.get(txn, &path[0]);
    }

    let last = path.pop().unwrap();
    let mut iter = path.into_iter();
    let mut map_ref = self.data.get(txn, &iter.next().unwrap())?.to_ymap();
    for path in iter {
      map_ref = map_ref?.get(txn, &path)?.to_ymap();
    }
    map_ref?.get(txn, &last)
  }

  pub fn remove(&mut self, key: &str) -> Option<Value> {
    let mut txn = self.transact_mut();
    self.data.remove(&mut txn, key)
  }

  pub fn remove_with_path<P: Into<Path>>(&mut self, path: P) -> Option<Value> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let len = path.len();
    if len == 1 {
      self.with_transact_mut(|txn| self.data.remove(txn, &path[0]))
    } else {
      let txn = self.transact();
      let mut iter = path.into_iter();
      let mut remove_path = iter.next().unwrap();
      let mut map_ref = self.data.get(&txn, &remove_path)?.to_ymap();

      let remove_index = len - 2;
      for (index, path) in iter.enumerate() {
        if index == remove_index {
          remove_path = path;
          break;
        } else {
          map_ref = map_ref?.get(&txn, &path)?.to_ymap();
        }
      }
      drop(txn);

      let map_ref = map_ref?;
      self.with_transact_mut(|txn| map_ref.remove(txn, &remove_path))
    }
  }

  pub fn to_json(&self) -> lib0::any::Any {
    let txn = self.transact();
    self.data.to_json(&txn)
  }

  pub fn to_json_value(&self) -> JsonValue {
    let txn = self.transact();
    serde_json::to_value(&self.data.to_json(&txn)).unwrap()
  }

  pub fn transact(&self) -> Transaction {
    self.doc.transact()
  }

  /// Returns a transaction that can mutate the document. This transaction will carry the
  /// origin of the current user.
  pub fn transact_mut(&self) -> TransactionMut {
    self.doc.transact_mut_with(self.origin.clone())
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = self.doc.transact_mut_with(self.origin.clone());
    let ret = f(&mut txn);
    drop(txn);
    ret
  }

  fn map_wrapper_with(&self, map_ref: MapRef) -> MapRefWrapper {
    MapRefWrapper::new(
      map_ref,
      CollabContext::new(self.origin.clone(), self.plugins.clone(), self.doc.clone()),
    )
  }
  fn array_wrapper_with(&self, array_ref: ArrayRef) -> ArrayRefWrapper {
    ArrayRefWrapper::new(
      array_ref,
      CollabContext::new(self.origin.clone(), self.plugins.clone(), self.doc.clone()),
    )
  }
}

/// Observe a document for updates.
/// Use the uid and the device_id to verify that the update is local or remote.
/// If the update is local, the plugins will be notified.
fn observe_doc(
  doc: &Doc,
  oid: String,
  plugins: Plugins,
  local_origin: ClientCollabOrigin,
) -> (UpdateSubscription, AfterTransactionSubscription) {
  let cloned_oid = oid.clone();
  let cloned_plugins = plugins.clone();
  let update_sub = doc
    .observe_update_v1(move |txn, event| {
      // If the origin of the txn is none, it means that the update is coming from a remote source.
      // Otherwise, it's a local update. The plugins only handle the local update
      if let Some(origin) = txn.origin().map(ClientCollabOrigin::from) {
        if origin == local_origin {
          cloned_plugins.read().iter().for_each(|plugin| {
            plugin.receive_local_update(&cloned_oid, txn, &event.update);
            plugin.did_receive_local_update(&origin, &cloned_oid, &event.update);
          });
        } else {
          let remote_origin = origin;
          tracing::trace!(
            "[🦀Client]: {} did apply remote {} update",
            local_origin,
            remote_origin,
          );
        }
      } else {
        tracing::trace!("[🦀Client]: did apply remote update");
      }
    })
    .unwrap();

  let after_txn_sub = doc
    .observe_after_transaction(move |txn| {
      plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.after_transaction(&oid, txn));
    })
    .unwrap();

  (update_sub, after_txn_sub)
}

impl Display for Collab {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&serde_json::to_string(self).unwrap())?;
    Ok(())
  }
}

/// A builder that used to create a new `Collab` instance.
pub struct CollabBuilder {
  uid: i64,
  plugins: Vec<Arc<dyn CollabPlugin>>,
  object_id: String,
}

impl CollabBuilder {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T) -> Self {
    let object_id = object_id.as_ref();
    Self {
      uid,
      plugins: vec![],
      object_id: object_id.to_string(),
    }
  }

  pub fn with_plugin<T>(mut self, plugin: T) -> Self
  where
    T: CollabPlugin + 'static,
  {
    self.plugins.push(Arc::new(plugin));
    self
  }

  pub fn build_with_updates(self, updates: Vec<Update>) -> Collab {
    let collab = Collab::new(self.uid, self.object_id, self.plugins);
    let mut txn = collab.transact_mut();
    for update in updates {
      txn.apply_update(update);
    }
    drop(txn);
    collab
  }

  pub fn build(self) -> Collab {
    Collab::new(self.uid, self.object_id, self.plugins)
  }
}

#[derive(Clone)]
pub struct CollabContext {
  origin: ClientCollabOrigin,
  doc: Doc,
  #[allow(dead_code)]
  plugins: Plugins,
}

impl CollabContext {
  fn new(origin: ClientCollabOrigin, plugins: Plugins, doc: Doc) -> Self {
    Self {
      origin,
      plugins,
      doc,
    }
  }

  pub fn transact(&self) -> Transaction {
    self.doc.transact()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = self.doc.transact_mut_with(self.origin.clone());
    let ret = f(&mut txn);
    drop(txn);
    ret
  }
}

#[derive(Clone)]
pub struct Path(Vec<String>);

impl IntoIterator for Path {
  type Item = String;
  type IntoIter = IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

impl From<Vec<&str>> for Path {
  fn from(values: Vec<&str>) -> Self {
    let values = values
      .into_iter()
      .map(|value| value.to_string())
      .collect::<Vec<String>>();
    Self(values)
  }
}

impl From<Vec<String>> for Path {
  fn from(values: Vec<String>) -> Self {
    Self(values)
  }
}

impl Deref for Path {
  type Target = Vec<String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Path {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Default, Clone)]
pub struct Plugins(Arc<RwLock<Vec<Arc<dyn CollabPlugin>>>>);

impl Plugins {
  pub fn new(plugins: Vec<Arc<dyn CollabPlugin>>) -> Plugins {
    Self(Arc::new(RwLock::new(plugins)))
  }
}

impl Deref for Plugins {
  type Target = Arc<RwLock<Vec<Arc<dyn CollabPlugin>>>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub trait CollabOrigin: Display + Serialize + Deserialize + Eq + PartialEq + Debug + Clone {}

/// This [ClientCollabOrigin] is used to verify the origin of a [Transaction] when
/// applying a remote update.
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct ClientCollabOrigin {
  pub uid: i64,
  pub device_id: Option<String>,
}

impl Default for ClientCollabOrigin {
  fn default() -> Self {
    Self {
      uid: 0,
      device_id: None,
    }
  }
}

impl Display for ClientCollabOrigin {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "[uid:{}|device_id:{}]",
      self.uid,
      self.device_id.as_ref().unwrap_or(&"".to_string())
    ))
  }
}

impl ClientCollabOrigin {
  pub fn new(uid: i64, device_id: &str) -> Self {
    Self {
      uid,
      device_id: Some(device_id.to_string()),
    }
  }
}

impl From<ClientCollabOrigin> for Origin {
  fn from(o: ClientCollabOrigin) -> Self {
    let data = serde_json::to_vec(&o).unwrap();
    Origin::from(data.as_slice())
  }
}

impl From<&Origin> for ClientCollabOrigin {
  fn from(value: &Origin) -> Self {
    serde_json::from_slice(value.as_ref()).unwrap_or_default()
  }
}

#[derive(Clone)]
pub struct MutexCollab(Arc<Mutex<Collab>>);

impl MutexCollab {
  pub fn new(
    origin: ClientCollabOrigin,
    object_id: &str,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Self {
    let collab = Collab::new_with_origin(origin, object_id, plugins);
    MutexCollab(Arc::new(Mutex::new(collab)))
  }

  pub fn initial(&self) {
    self.0.lock().initial();
  }

  pub fn to_json_value(&self) -> JsonValue {
    self.0.lock().to_json_value()
  }
}

impl Deref for MutexCollab {
  type Target = Arc<Mutex<Collab>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexCollab {}

unsafe impl Send for MutexCollab {}
