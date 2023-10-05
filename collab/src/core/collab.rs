use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::vec::IntoIter;

use bytes::Bytes;
use parking_lot::{Mutex, RwLock};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_stream::wrappers::WatchStream;
use yrs::block::Prelim;
use yrs::types::map::MapEvent;
use yrs::types::{ToJson, Value};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
  ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Observable, Options, ReadTxn, StateVector,
  Subscription, Transact, Transaction, TransactionMut, UndoManager, Update, UpdateSubscription,
};

use crate::core::collab_plugin::{CollabPlugin, CollabPluginType};
use crate::core::collab_state::{InitState, SnapshotState, State, SyncState};
use crate::core::map_wrapper::{CustomMapRef, MapRefWrapper};
use crate::core::origin::{CollabClient, CollabOrigin};
use crate::core::transaction::TransactionRetry;
use crate::error::CollabError;
use crate::preclude::{ArrayRefWrapper, JsonValue, MapRefExtension};
use crate::sync_protocol::awareness::Awareness;
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

  /// This [CollabClient] is used to verify the origin of a [Transaction] when
  /// applying a remote update.
  origin: CollabOrigin,

  /// The [Doc] is the main data structure that is used to store the data.
  doc: Doc,
  /// The [Awareness] is used to track the awareness of the other peers.
  awareness: Awareness,

  /// Every [Collab] instance has a data section that can be used to store
  data: MapRef,

  /// A list of plugins that are used to extend the functionality of the [Collab].
  plugins: Plugins,

  state: Arc<State>,

  /// The [UndoManager] is used to undo and redo changes. By default, the [UndoManager]
  /// is disabled. To enable it, call [Collab::enable_undo_manager].
  undo_manager: Mutex<Option<UndoManager>>,
  update_subscription: RwLock<Option<UpdateSubscription>>,
  after_txn_subscription: RwLock<Option<AfterTransactionSubscription>>,
}

impl Collab {
  pub fn new<T: AsRef<str>>(
    uid: i64,
    object_id: T,
    device_id: impl ToString,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Collab {
    let origin = CollabClient::new(uid, device_id);
    Self::new_with_origin(CollabOrigin::Client(origin), object_id, plugins)
  }

  pub fn new_with_raw_data(
    origin: CollabOrigin,
    object_id: &str,
    collab_raw_data: CollabRawData,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Result<Self, CollabError> {
    let collab = Self::new_with_origin(origin, object_id, plugins);
    if !collab_raw_data.is_empty() {
      let mut txn = collab.origin_transact_mut();
      for update in collab_raw_data {
        let decoded_update = Update::decode_v1(&update)?;
        txn.try_apply_update(decoded_update)?;
      }
    }
    Ok(collab)
  }

  pub fn new_with_origin<T: AsRef<str>>(
    origin: CollabOrigin,
    object_id: T,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Collab {
    let object_id = object_id.as_ref().to_string();
    let doc = Doc::with_options(Options {
      skip_gc: true,
      ..Options::default()
    });
    let data = doc.get_or_insert_map(DATA_SECTION);
    let undo_manager = Mutex::new(None);
    let plugins = Plugins::new(plugins);
    let state = Arc::new(State::new(&object_id));
    let awareness = Awareness::new(doc.clone());

    Self {
      origin,
      object_id,
      doc,
      undo_manager,
      awareness,
      data,
      plugins,
      state,
      update_subscription: Default::default(),
      after_txn_subscription: Default::default(),
    }
  }

  pub fn encode_as_update_v1(&self) -> (Vec<u8>, Vec<u8>) {
    let txn = self.transact();
    (
      txn.encode_state_as_update_v1(&StateVector::default()),
      txn.state_vector().encode_v1(),
    )
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    WatchStream::new(self.state.sync_state_notifier.subscribe())
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    WatchStream::new(self.state.snapshot_state_notifier.subscribe())
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
    self.add_plugins(vec![plugin]);
  }

  /// Add plugins to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugins(&mut self, plugins: Vec<Arc<dyn CollabPlugin>>) {
    let mut write_guard = self.plugins.write();

    for plugin in plugins {
      if plugin.plugin_type() == CollabPluginType::CloudStorage {
        let is_exist = write_guard
          .iter()
          .find(|plugin| plugin.plugin_type() == CollabPluginType::CloudStorage);
        if is_exist.is_some() {
          tracing::error!("Only one cloud storage plugin can be added to a collab instance.");
        }
      }
      write_guard.push(plugin);
    }
  }

  /// Upon calling this method, the [Collab]'s document will be initialized with the plugins. The callbacks from the plugins
  /// will be triggered in the order they were added. The input parameter, [init_sync], indicates whether the
  /// [Collab] is initialized with local data or remote updates. If true, it suggests that the data doesn't need
  /// further synchronization with the remote server.
  ///
  /// This method must be called after all plugins have been added.
  pub fn initialize(&self) {
    if !self.state.is_uninitialized() {
      return;
    }

    self.state.set_init_state(InitState::Loading);
    {
      let plugins = self.plugins.read().clone();
      for plugin in plugins {
        plugin.init(&self.object_id, &self.origin, &self.doc);
      }
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
      self
        .plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.did_init(&self.awareness, &self.object_id));
    }
    self.state.set_init_state(InitState::Initialized);
  }

  pub fn set_sync_state(&self, sync_state: SyncState) {
    self.state.set_sync_state(sync_state);
  }

  pub fn set_snapshot_state(&self, snapshot_state: SnapshotState) {
    self.state.set_snapshot_state(snapshot_state);
  }

  pub fn reset(&self) {
    self
      .plugins
      .read()
      .iter()
      .for_each(|plugin| plugin.reset(&self.object_id));
  }

  /// Make a full update with the current state of the [Collab].
  /// It invokes the [CollabPlugin::flush] method of each plugin.
  pub fn flush(&self) {
    let update = Bytes::from(self.encode_as_update_v1().0);
    self
      .plugins
      .read()
      .iter()
      .for_each(|plugin| plugin.flush(&self.object_id, &update));
  }

  pub fn observer_data<F>(&mut self, f: F) -> MapSubscription
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
    self.with_origin_transact_mut(|txn| self.insert_with_txn(txn, key, value))
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

    self.with_origin_transact_mut(|txn| {
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

  pub fn insert_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRefWrapper {
    let map = MapPrelim::<lib0::any::Any>::new();
    let map_ref = self.data.insert(txn, key, map);
    self.map_wrapper_with(map_ref)
  }

  pub fn insert_map_with_txn_if_not_exist(
    &self,
    txn: &mut TransactionMut,
    key: &str,
  ) -> MapRefWrapper {
    let map_ref = self.data.create_map_if_not_exist_with_txn(txn, key);
    self.map_wrapper_with(map_ref)
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
    let mut txn = self.origin_transact_mut();
    self.data.remove(&mut txn, key)
  }

  pub fn remove_with_path<P: Into<Path>>(&mut self, path: P) -> Option<Value> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let len = path.len();
    if len == 1 {
      self.with_origin_transact_mut(|txn| self.data.remove(txn, &path[0]))
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
      self.with_origin_transact_mut(|txn| map_ref.remove(txn, &remove_path))
    }
  }

  pub fn to_json(&self) -> lib0::any::Any {
    let txn = self.transact();
    self.data.to_json(&txn)
  }

  pub fn to_plain_text(&self) -> String {
    "".to_string()
  }

  pub fn to_json_value(&self) -> JsonValue {
    let txn = self.transact();
    serde_json::to_value(&self.data.to_json(&txn)).unwrap()
  }

  pub fn enable_undo_redo(&mut self) {
    if self.undo_manager.lock().is_some() {
      tracing::warn!("Undo manager already enabled");
      return;
    }
    // a frequent case includes establishing a new transaction for every user key stroke. Meanwhile
    // we may decide to use different granularity of undo/redo actions. These are grouped together
    // on time-based ranges (configurable in undo::Options, which is 500ms by default).
    let mut undo_manager =
      UndoManager::with_options(&self.doc, &self.data, yrs::undo::Options::default());
    undo_manager.include_origin(self.origin.clone());
    *self.undo_manager.lock() = Some(undo_manager);
  }

  /// Undo the previous change.
  /// Returns true if the undo was successful, false if there was nothing to undo. If the
  /// UndoManager is not enabled, returns false.
  pub fn can_undo(&self) -> bool {
    match &*self.undo_manager.lock() {
      None => {
        tracing::warn!("Undo manager not enabled, should enable_undo_redo first");
        false
      },
      Some(undo_mgr) => undo_mgr.can_undo(),
    }
  }

  /// Redo the previous change.
  /// Returns true if the redo was successful, false if there was nothing to redo. If the
  /// UndoManager is not enabled, returns false.
  pub fn can_redo(&self) -> bool {
    match &*self.undo_manager.lock() {
      None => {
        tracing::warn!("Undo manager not enabled, should enable_undo_redo first");
        false
      },
      Some(undo_mgr) => undo_mgr.can_redo(),
    }
  }

  pub fn undo(&mut self) -> Result<bool, CollabError> {
    match &mut *self.undo_manager.lock() {
      None => Err(CollabError::UndoManagerNotEnabled),
      Some(mgr) => mgr.undo().map_err(|e| CollabError::Internal(Box::new(e))),
    }
  }

  pub fn redo(&mut self) -> Result<bool, CollabError> {
    match &mut *self.undo_manager.lock() {
      None => Err(CollabError::UndoManagerNotEnabled),
      Some(mgr) => mgr.redo().map_err(|e| CollabError::Internal(Box::new(e))),
    }
  }

  pub fn transact(&self) -> Transaction {
    TransactionRetry::new(&self.doc).get_read_txn()
  }

  pub fn try_transaction(&self) -> Result<Transaction, CollabError> {
    self
      .doc
      .try_transact()
      .map_err(|e| CollabError::Internal(Box::new(e)))
  }

  pub fn try_transaction_mut(&self) -> Result<TransactionMut, CollabError> {
    TransactionRetry::new(&self.doc).try_get_write_txn()
  }

  pub fn try_origin_transaction_mut(&self) -> Result<TransactionMut, CollabError> {
    TransactionRetry::new(&self.doc).try_get_write_txn_with(self.origin.clone())
  }

  /// Returns a transaction that can mutate the document. This transaction will carry the
  /// origin of the current user.
  pub fn origin_transact_mut(&self) -> TransactionMut {
    TransactionRetry::new(&self.doc).get_write_txn_with(self.origin.clone())
  }

  /// Returns a transaction that can mutate the document. This transaction will carry the
  /// origin of the current user.
  ///
  /// If applying the remote update, please use the `transact_mut` of `doc`. Ot
  /// update will send to remote that the remote already has.
  pub fn with_origin_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = TransactionRetry::new(&self.doc).get_write_txn_with(self.origin.clone());
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
  local_origin: CollabOrigin,
) -> (UpdateSubscription, AfterTransactionSubscription) {
  let cloned_oid = oid.clone();
  let cloned_plugins = plugins.clone();
  let update_sub = doc
    .observe_update_v1(move |txn, event| {
      // If the origin of the txn is none, it means that the update is coming from a remote source.
      cloned_plugins.read().iter().for_each(|plugin| {
        plugin.receive_update(&cloned_oid, txn, &event.update);

        let remote_origin = CollabOrigin::from(txn);
        if remote_origin == local_origin {
          plugin.receive_local_update(&local_origin, &cloned_oid, &event.update);
        } else {
          tracing::trace!(
            "[🙂Client]: {} did apply remote {} update",
            local_origin,
            remote_origin,
          );
        }
      });
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
  device_id: String,
  plugins: Vec<Arc<dyn CollabPlugin>>,
  object_id: String,
  updates: CollabRawData,
}

/// The raw data of a collab document. It is a list of updates. Each of them can be parsed by
/// [Update::decode_v1].
pub type CollabRawData = Vec<Vec<u8>>;

impl CollabBuilder {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T) -> Self {
    let object_id = object_id.as_ref();
    Self {
      uid,
      plugins: vec![],
      object_id: object_id.to_string(),
      device_id: "".to_string(),
      updates: vec![],
    }
  }

  pub fn with_device_id<T>(mut self, device_id: T) -> Self
  where
    T: AsRef<str>,
  {
    self.device_id = device_id.as_ref().to_string();
    self
  }

  pub fn with_plugin<T>(mut self, plugin: T) -> Self
  where
    T: CollabPlugin + 'static,
  {
    self.plugins.push(Arc::new(plugin));
    self
  }

  pub fn with_raw_data(mut self, updates: Vec<Vec<u8>>) -> Self {
    self.updates = updates;
    self
  }

  pub fn build(self) -> Result<MutexCollab, CollabError> {
    let origin = CollabOrigin::Client(CollabClient::new(self.uid, self.device_id));
    MutexCollab::new_with_raw_data(origin, &self.object_id, self.updates, self.plugins)
  }
}

#[derive(Clone)]
pub struct CollabContext {
  origin: CollabOrigin,
  doc: Doc,
  #[allow(dead_code)]
  plugins: Plugins,
}

impl CollabContext {
  fn new(origin: CollabOrigin, plugins: Plugins, doc: Doc) -> Self {
    Self {
      origin,
      plugins,
      doc,
    }
  }

  pub fn transact(&self) -> Transaction {
    TransactionRetry::new(&self.doc).get_read_txn()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = TransactionRetry::new(&self.doc).get_write_txn_with(self.origin.clone());
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

#[derive(Clone)]
pub struct MutexCollab(Arc<Mutex<Collab>>);

impl MutexCollab {
  pub fn new(origin: CollabOrigin, object_id: &str, plugins: Vec<Arc<dyn CollabPlugin>>) -> Self {
    let collab = Collab::new_with_origin(origin, object_id, plugins);
    MutexCollab(Arc::new(Mutex::new(collab)))
  }

  pub fn new_with_raw_data(
    origin: CollabOrigin,
    object_id: &str,
    collab_raw_data: CollabRawData,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Result<Self, CollabError> {
    let collab = Collab::new_with_raw_data(origin, object_id, collab_raw_data, plugins)?;
    Ok(MutexCollab(Arc::new(Mutex::new(collab))))
  }

  pub fn from_collab(collab: Collab) -> Self {
    MutexCollab(Arc::new(Mutex::new(collab)))
  }

  pub fn encode_as_update_v1(&self) -> (Vec<u8>, Vec<u8>) {
    let collab = self.0.lock();
    collab.encode_as_update_v1()
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

// Extension trait for `TransactionMut`
pub trait TransactionMutExt<'doc> {
  /// Applies an update to the document. If the update is invalid, it will return an error.
  /// It allows to catch panics from `apply_update`.
  fn try_apply_update(&mut self, update: Update) -> Result<(), CollabError>;
}

impl<'doc> TransactionMutExt<'doc> for TransactionMut<'doc> {
  fn try_apply_update(&mut self, update: Update) -> Result<(), CollabError> {
    match panic::catch_unwind(AssertUnwindSafe(|| {
      self.apply_update(update);
    })) {
      Ok(_) => Ok(()),
      Err(e) => Err(CollabError::YrsTransactionError(format!("{:?}", e))),
    }
  }
}
