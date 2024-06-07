use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::panic::AssertUnwindSafe;

use arc_swap::ArcSwapOption;
use std::sync::Arc;
use std::vec::IntoIter;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use tokio_stream::wrappers::WatchStream;
use tracing::{error, instrument, trace};
use yrs::block::Prelim;
use yrs::types::map::MapEvent;
use yrs::types::{ToJson, Value};
use yrs::updates::decoder::Decode;

use yrs::updates::encoder::Encode;
use yrs::{
  Any, ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Observable, OffsetKind, Options,
  ReadTxn, StateVector, Subscription, Transact, Transaction as Tx, TransactionMut as TxMut,
  UndoManager, Update,
};

use crate::core::awareness::{Awareness, Event};
use crate::core::collab_plugin::{CollabPlugin, Plugins};
use crate::core::collab_state::{InitState, SnapshotState, State, SyncState};
use crate::core::map_wrapper::{CustomMapRef, MapRefWrapper};
use crate::core::origin::{CollabClient, CollabOrigin};
use crate::core::transaction::{DocTransactionExtension, Transaction, TransactionMut};
use crate::core::value::YrsValueExtension;
use crate::entity::EncodedCollab;
use crate::error::CollabError;
use crate::preclude::{ArrayRefWrapper, JsonValue, MapRefExtension};
use crate::util::insert_json_value_to_map_ref;

pub const DATA_SECTION: &str = "data";
pub const META_SECTION: &str = "meta";

const LAST_SYNC_AT: &str = "last_sync_at";

type AfterTransactionSubscription = Subscription;

pub type MapSubscriptionCallback = Arc<dyn Fn(&TxMut, &MapEvent)>;
pub type MapSubscription = Subscription;

#[derive(Debug, Clone)]
pub enum IndexContent {
  Create(serde_json::Value),
  Update(serde_json::Value),
  Delete(Vec<String>),
}
pub type IndexContentSender = tokio::sync::broadcast::Sender<IndexContent>;
pub type IndexContentReceiver = tokio::sync::broadcast::Receiver<IndexContent>;
/// A [Collab] is a wrapper around a [Doc] and [Awareness] that provides a set
/// of helper methods for interacting with the [Doc] and [Awareness]. The [MutexCollab]
/// is a thread-safe wrapper around the [Collab].
pub struct Collab {
  /// The object id can be the document id or the database id. It must be unique for
  /// each [Collab] instance.
  object_id: String,
  /// This [CollabClient] is used to verify the origin of a [Transaction] when
  /// applying a remote update.
  origin: CollabOrigin,
  /// Every [Collab] instance has a data section that can be used to store
  data: MapRef,
  meta: MapRef,
  state: Arc<State>,
  update_subscription: ArcSwapOption<Subscription>,
  awareness_subscription: ArcSwapOption<Subscription>,
  after_txn_subscription: ArcSwapOption<AfterTransactionSubscription>,
  index_json_sender: IndexContentSender,
  /// A list of plugins that are used to extend the functionality of the [Collab].
  plugins: Plugins,
  inner: RwLock<CollabInner>,
}

unsafe impl Send for Collab {} // TODO: Remove this once MapRefs are Send
unsafe impl Sync for Collab {} // TODO: Remove this once MapRefs are Sync

pub struct CollabInner {
  /// The [Doc] is the main data structure that is used to store the data.
  pub doc: Doc,
  /// The [Awareness] is used to track the awareness of the other peers.
  pub awareness: Awareness,
  /// The [UndoManager] is used to undo and redo changes. By default, the [UndoManager]
  /// is disabled. To enable it, call [Collab::enable_undo_manager].
  pub undo_manager: Option<UndoManager>,
}

impl CollabInner {
  pub fn doc(&self) -> &Doc {
    &self.doc
  }

  pub fn doc_mut(&mut self) -> &Doc {
    &mut self.doc
  }

  pub fn awareness(&self) -> &Awareness {
    &self.awareness
  }

  pub fn awareness_mut(&mut self) -> &mut Awareness {
    &mut self.awareness
  }

  pub fn undo_manager(&self) -> Result<&UndoManager, CollabError> {
    match &self.undo_manager {
      None => Err(CollabError::UndoManagerNotEnabled),
      Some(mgr) => Ok(mgr),
    }
  }

  pub fn undo_manager_mut(&mut self) -> Result<&mut UndoManager, CollabError> {
    match &mut self.undo_manager {
      None => Err(CollabError::UndoManagerNotEnabled),
      Some(mgr) => Ok(mgr),
    }
  }
}

pub fn make_yrs_doc(skp_gc: bool) -> Doc {
  Doc::with_options(Options {
    skip_gc: skp_gc,
    offset_kind: OffsetKind::Utf16,
    ..Options::default()
  })
}

impl Collab {
  pub fn new<T: AsRef<str>>(
    uid: i64,
    object_id: T,
    device_id: impl ToString,
    plugins: Vec<Box<dyn CollabPlugin>>,
    skip_gc: bool,
  ) -> Collab {
    let origin = CollabClient::new(uid, device_id);
    Self::new_with_origin(CollabOrigin::Client(origin), object_id, plugins, skip_gc)
  }

  pub fn new_with_source(
    origin: CollabOrigin,
    object_id: &str,
    collab_doc_state: DataSource,
    plugins: Vec<Box<dyn CollabPlugin>>,
    skip_gc: bool,
  ) -> Result<Self, CollabError> {
    let collab = Self::new_with_origin(origin, object_id, plugins, skip_gc);
    match collab_doc_state {
      DataSource::Disk => {},
      DataSource::DocStateV1(doc_state) => {
        if !doc_state.is_empty() {
          let mut txn = collab.origin_transact_mut();
          let decoded_update = Update::decode_v1(&doc_state)?;
          txn.try_apply_update(decoded_update)?;
        }
      },
      DataSource::DocStateV2(doc_state) => {
        if !doc_state.is_empty() {
          let mut txn = collab.origin_transact_mut();
          let decoded_update = Update::decode_v2(&doc_state)?;
          txn.try_apply_update(decoded_update)?;
        }
      },
    }

    Ok(collab)
  }

  pub fn clear_plugins(&mut self) {
    let plugins = self.plugins.remove_all();
    for plugin in plugins {
      plugin.destroy();
    }
  }

  pub fn new_with_origin<T: AsRef<str>>(
    origin: CollabOrigin,
    object_id: T,
    plugins: Vec<Box<dyn CollabPlugin>>,
    skip_gc: bool,
  ) -> Collab {
    let object_id = object_id.as_ref().to_string();
    let doc = make_yrs_doc(skip_gc);
    let data = doc.get_or_insert_map(DATA_SECTION);
    let meta = doc.get_or_insert_map(META_SECTION);
    let undo_manager = None;
    let plugins = Plugins::new(plugins);
    let state = Arc::new(State::new(&object_id));
    let awareness = Awareness::new(doc.clone());
    Self {
      origin,
      object_id,
      inner: RwLock::new(CollabInner {
        doc,
        awareness,
        undo_manager,
      }),
      state,
      data,
      meta,
      plugins,
      update_subscription: Default::default(),
      after_txn_subscription: Default::default(),
      awareness_subscription: Default::default(),
      index_json_sender: tokio::sync::broadcast::channel(100).0,
    }
  }

  pub async fn transaction<'a>(&'a self) -> Transaction<'a> {
    let lock: RwLockReadGuard<'a, CollabInner> = self.inner.read().await;
    Transaction::new(lock)
  }

  pub async fn transaction_mut<'a>(&'a self) -> TransactionMut<'a> {
    let lock: RwLockWriteGuard<'a, CollabInner> = self.inner.write().await;
    TransactionMut::new(lock, self.origin.clone())
  }

  /// Returns the doc state and the state vector.
  pub async fn encode_collab_v1<F, E>(&self, validate: F) -> Result<EncodedCollab, E>
  where
    F: FnOnce(&Collab) -> Result<(), E>,
    E: std::fmt::Debug,
  {
    validate(self)?;
    let lock = self.inner.read().await;
    let tx = lock.doc.transact();
    Ok(tx.get_encoded_collab_v1())
  }

  pub fn try_encode_collab_v1<F, E>(&self, validate: F) -> Result<EncodedCollab, CollabError>
  where
    F: FnOnce(&Collab) -> Result<(), E>,
    E: std::fmt::Debug,
  {
    validate(self).map_err(|err| CollabError::NoRequiredData(format!("{:?}", err)))?;

    let lock = self
      .inner
      .try_read()
      .map_err(|_| CollabError::AcquiredReadTxnFail)?;

    let tx = lock.doc.transact();
    Ok(EncodedCollab::new_v1(
      tx.state_vector().encode_v1(),
      tx.encode_state_as_update_v1(&StateVector::default()),
    ))
  }

  pub async fn encode_collab_v2(&self) -> EncodedCollab {
    let lock = self.inner.read().await;
    let tx = lock.doc.transact();
    tx.get_encoded_collab_v2()
  }

  pub fn get_state(&self) -> &Arc<State> {
    &self.state
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    WatchStream::new(self.state.sync_state_notifier.subscribe())
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    WatchStream::new(self.state.snapshot_state_notifier.subscribe())
  }

  pub async fn clean_awareness_state(&mut self) {
    let mut lock = self.inner.write().await;
    lock.awareness.clean_local_state();
  }

  pub async fn emit_awareness_state(&mut self) {
    if let CollabOrigin::Client(origin) = &self.origin {
      let mut lock = self.inner.write().await;
      lock
        .awareness
        .set_local_state(initial_awareness_state(origin.uid).to_string());
    }
  }

  /// Subscribes to the `IndexJson` associated with a `Collab` object.
  ///
  /// `IndexJson` is a JSON object containing data used for indexing purposes. The structure and
  /// content of this data may vary between different collaborative objects derived from `Collab`.
  /// The interpretation of `IndexJson` is specific to the subscriber, as only they know how to
  /// process and utilize the contained indexing information.
  pub fn subscribe_index_content(&self) -> IndexContentReceiver {
    self.index_json_sender.subscribe()
  }

  #[inline]
  pub async fn into_inner(&self) -> RwLockReadGuard<CollabInner> {
    self.inner.read().await
  }

  #[inline]
  pub async fn into_inner_mut(&self) -> RwLockWriteGuard<CollabInner> {
    self.inner.write().await
  }

  /// Add a plugin to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugin(&mut self, plugin: Box<dyn CollabPlugin>) {
    self.add_plugins([plugin]);
  }

  /// Add plugins to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugins<I>(&mut self, plugins: I)
  where
    I: IntoIterator<Item = Box<dyn CollabPlugin>>,
  {
    for plugin in plugins.into_iter() {
      if !self.plugins.push_front(plugin) {
        tracing::error!("Only one cloud storage plugin can be added to a collab instance.");
      }
    }
  }

  /// Upon calling this method, the [Collab]'s document will be initialized with the plugins. The callbacks from the plugins
  /// will be triggered in the order they were added. The input parameter, [init_sync], indicates whether the
  /// [Collab] is initialized with local data or remote updates. If true, it suggests that the data doesn't need
  /// further synchronization with the remote server.
  ///
  /// This method must be called after all plugins have been added.
  pub async fn initialize(&mut self) {
    if !self.state.is_uninitialized() {
      return;
    }

    let mut lock = self.inner.write().await;
    self.state.set_init_state(InitState::Loading);
    {
      for plugin in self.plugins.iter() {
        plugin.init(&self.object_id, &self.origin, &lock.doc);
      }
    }

    let (update_subscription, after_txn_subscription) = observe_doc(
      &lock.doc,
      self.object_id.clone(),
      self.plugins.clone(),
      self.origin.clone(),
    );

    let awareness_subscription = observe_awareness(
      &mut lock.awareness,
      self.plugins.clone(),
      self.object_id.clone(),
      self.origin.clone(),
    );

    self
      .update_subscription
      .store(Some(update_subscription.into()));
    self
      .after_txn_subscription
      .store(after_txn_subscription.map(Arc::from));
    self
      .awareness_subscription
      .store(Some(awareness_subscription.into()));

    let last_sync_at = self.get_last_sync_at();
    {
      self
        .plugins
        .iter()
        .for_each(|plugin| plugin.did_init(self, &self.object_id, last_sync_at));
    }
    self.state.set_init_state(InitState::Initialized);
  }

  pub fn set_last_sync_at(&self, last_sync_at: i64) {
    match self.inner.try_read() {
      Ok(inner) => {
        let mut txn = inner.doc.transact_mut();
        self.set_last_sync_at_with_txn(&mut txn, last_sync_at)
      },
      Err(_) => {},
    }
  }

  pub fn set_last_sync_at_with_txn(&self, txn: &mut TxMut, last_sync_at: i64) {
    //FIXME: this is very expensive to do on frequent basis. That's one of the reasons we have
    // awareness state separate from document
    self
      .meta
      .insert_i64_with_txn(txn, LAST_SYNC_AT, last_sync_at);
  }

  pub fn get_last_sync_at(&self) -> i64 {
    match self.try_transaction() {
      Ok(txn) => self.meta.get_i64_with_txn(&txn, LAST_SYNC_AT).unwrap_or(0),
      Err(_) => 0,
    }
  }

  pub fn set_sync_state(&self, sync_state: SyncState) {
    self.state.set_sync_state(sync_state);
  }

  pub fn set_snapshot_state(&self, snapshot_state: SnapshotState) {
    self.state.set_snapshot_state(snapshot_state);
  }

  /// Make a full update with the current state of the [Collab].
  /// It invokes the [CollabPlugin::flush] method of each plugin.
  pub async fn flush(&self) {
    let lock = self.inner.write().await;
    for plugin in self.plugins.iter() {
      plugin.flush(&self.object_id, &lock.doc)
    }
  }

  pub fn observe_data<F>(&mut self, f: F) -> MapSubscription
  where
    F: Fn(&TransactionMut, &MapEvent) + 'static,
  {
    self.data.observe(f)
  }

  pub async fn observe_awareness<F>(&mut self, f: F) -> Subscription
  where
    F: Fn(&Event) + 'static,
  {
    let lock = self.inner.write().await;
    lock.awareness.on_update(f)
  }

  pub async fn get(&self, key: &str) -> Option<Value> {
    let lock = self.inner.read().await;
    let txn = lock.doc.transact();
    self.data.get(&txn, key)
  }

  pub fn get_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<Value> {
    self.data.get(txn, key)
  }

  pub async fn insert<V: Prelim>(&self, key: &str, value: V) -> V::Return {
    let mut tx = self.transaction_mut().await;
    self.insert_with_txn(&mut tx, key, value)
  }

  pub fn insert_with_txn<V: Prelim>(&self, txn: &mut TxMut, key: &str, value: V) -> V::Return {
    self.data.insert(txn, key, value)
  }

  pub async fn insert_json_with_path<T: Serialize>(
    &mut self,
    path: Vec<String>,
    key: &str,
    value: T,
  ) {
    let mut txn = self.transaction_mut().await;
    let mut map = if path.is_empty() {
      None
    } else {
      self
        .get_map_with_txn(txn.deref(), path)
        .map(|m| m.into_inner())
    };

    if map.is_none() {
      map = Some(self.data.insert(&mut txn, key, MapPrelim::<Any>::new()));
    }
    let value = serde_json::to_value(&value).unwrap();
    insert_json_value_to_map_ref(key, &value, map.unwrap(), &mut txn);
  }

  pub async fn get_json_with_path<T: DeserializeOwned>(&self, path: impl Into<Path>) -> Option<T> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let map = self.get_map_with_txn(self.transaction().await.deref(), path)?;

    let json_str = map.to_json_str();
    let object = serde_json::from_str::<T>(&json_str).ok()?;
    Some(object)
  }

  pub fn insert_map_with_txn(&self, txn: &mut TxMut, key: &str) -> MapRefWrapper {
    let map = MapPrelim::<Any>::new();
    let map_ref = self.data.insert(txn, key, map);
    self.map_wrapper_with(map_ref)
  }

  pub fn insert_map_with_txn_if_not_exist(&self, txn: &mut TxMut, key: &str) -> MapRefWrapper {
    let map_ref = self.data.create_map_if_not_exist_with_txn(txn, key);
    self.map_wrapper_with(map_ref)
  }

  pub async fn get_map_with_path<M: CustomMapRef>(&self, path: impl Into<Path>) -> Option<M> {
    let lock = self.inner.read().await;
    let txn = lock.doc.transact();
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
    let value = self.data.get(txn, &iter.next().unwrap())?;
    let mut map_ref = value.to_ymap().cloned();
    for path in iter {
      map_ref = map_ref?.get(txn, &path)?.to_ymap().cloned();
    }
    map_ref.map(|map_ref| self.map_wrapper_with(map_ref))
  }

  pub fn get_array_with_txn<P: Into<Path>, T: ReadTxn>(
    &self,
    txn: &T,
    path: P,
  ) -> Option<ArrayRefWrapper> {
    let path = path.into();
    let value = self.get_ref_from_path_with_txn(txn, path)?;
    let array_ref = value.to_yarray();
    array_ref.map(|array_ref| self.array_wrapper_with(array_ref.clone()))
  }

  pub fn create_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TxMut,
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
    let mut map_ref = self
      .data
      .get(txn, &iter.next().unwrap())?
      .to_ymap()
      .cloned();
    for path in iter {
      map_ref = map_ref?.get(txn, &path)?.to_ymap().cloned();
    }
    map_ref?.get(txn, &last)
  }

  pub async fn remove(&mut self, key: &str) -> Option<Value> {
    let mut txn = self.transaction_mut().await;
    self.data.remove(&mut txn, key)
  }

  pub async fn remove_with_path<P: Into<Path>>(&mut self, path: P) -> Option<Value> {
    let path = path.into();
    if path.is_empty() {
      return None;
    }
    let mut txn = self.transaction_mut().await;
    let len = path.len();
    if len == 1 {
      self.data.remove(&mut txn, &path[0])
    } else {
      let mut iter = path.into_iter();
      let mut remove_path = iter.next().unwrap();
      let mut map_ref = self.data.get(txn.deref(), &remove_path)?.to_ymap().cloned();

      let remove_index = len - 2;
      for (index, path) in iter.enumerate() {
        if index == remove_index {
          remove_path = path;
          break;
        } else {
          map_ref = map_ref?.get(txn.deref(), &path)?.to_ymap().cloned();
        }
      }

      let map_ref = map_ref?;
      map_ref.remove(&mut txn, &remove_path)
    }
  }

  pub fn to_json(&self) -> Any {
    let txn = self.transact();
    self.data.to_json(&txn)
  }

  pub fn to_plain_text(&self) -> String {
    "".to_string()
  }

  pub async fn to_json_value(&self) -> JsonValue {
    let txn = self.transaction().await;
    serde_json::to_value(&self.data.to_json(txn.deref())).unwrap()
  }

  pub async fn enable_undo_redo(&self) {
    let mut lock = self.inner.write().await;
    if lock.undo_manager.is_some() {
      tracing::warn!("Undo manager already enabled");
      return;
    }
    // a frequent case includes establishing a new transaction for every user key stroke. Meanwhile
    // we may decide to use different granularity of undo/redo actions. These are grouped together
    // on time-based ranges (configurable in undo::Options, which is 500ms by default).
    let mut undo_manager =
      UndoManager::with_options(&lock.doc, &self.data, yrs::undo::Options::default());
    undo_manager.include_origin(self.origin.clone());
    lock.undo_manager = Some(undo_manager);
  }

  pub fn start_init_sync(&self) {
    self.plugins.read().iter().for_each(|plugin| {
      plugin.start_init_sync();
    });
  }

  fn map_wrapper_with(&self, map_ref: MapRef) -> MapRefWrapper {
    MapRefWrapper::new(
      map_ref,
      CollabContext::new(
        self.origin.clone(),
        self.plugins.clone(),
        self.doc.clone(),
        self.object_id.clone(),
      ),
    )
  }
  fn array_wrapper_with(&self, array_ref: ArrayRef) -> ArrayRefWrapper {
    ArrayRefWrapper::new(
      array_ref,
      CollabContext::new(
        self.origin.clone(),
        self.plugins.clone(),
        self.doc.clone(),
        self.object_id.clone(),
      ),
    )
  }
}

fn observe_awareness(
  awareness: &mut Awareness,
  plugins: Plugins,
  oid: String,
  origin: CollabOrigin,
) -> Subscription {
  awareness.on_update(move |event| {
    if let Ok(update) = event.awareness_state().full_update() {
      plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.receive_local_state(&origin, &oid, event, &update));
    }
  })
}

/// Observe a document for updates.
/// Use the uid and the device_id to verify that the update is local or remote.
/// If the update is local, the plugins will be notified.
fn observe_doc(
  doc: &Doc,
  oid: String,
  plugins: Plugins,
  local_origin: CollabOrigin,
) -> (Subscription, Option<AfterTransactionSubscription>) {
  let cloned_oid = oid.clone();
  let cloned_plugins = plugins.clone();
  let update_sub = doc
    .observe_update_v1(move |txn, event| {
      // If the origin of the txn is none, it means that the update is coming from a remote source.
      for plugin in cloned_plugins.iter() {
        plugin.receive_update(&cloned_oid, txn, &event.update);

        let remote_origin = CollabOrigin::from(txn);
        if remote_origin == local_origin {
          plugin.receive_local_update(&local_origin, &cloned_oid, &event.update);
        } else {
          #[cfg(feature = "verbose_log")]
          trace!("{} did apply remote {} update", local_origin, remote_origin);
        }
      }
    })
    .unwrap();

  let after_txn_sub = doc
    .observe_after_transaction(move |txn| {
      for plugin in plugins.iter() {
        plugin.after_transaction(&oid, txn)
      }
    })
    .ok();

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
  plugins: Vec<Box<dyn CollabPlugin>>,
  object_id: String,
  source: DataSource,
  skip_gc: bool,
}

/// The raw data of a collab document. It is a list of updates. Each of them can be parsed by
/// [Update::decode_v1].
pub enum DataSource {
  Disk,
  DocStateV1(Vec<u8>),
  DocStateV2(Vec<u8>),
}

impl DataSource {
  pub fn is_empty(&self) -> bool {
    matches!(self, DataSource::Disk)
  }
}
impl CollabBuilder {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T) -> Self {
    let object_id = object_id.as_ref();
    Self {
      uid,
      plugins: vec![],
      object_id: object_id.to_string(),
      device_id: "".to_string(),
      source: DataSource::Disk,
      skip_gc: true,
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
    self.plugins.push(Box::new(plugin));
    self
  }

  pub fn with_doc_state(mut self, doc_state: DataSource) -> Self {
    self.source = doc_state;
    self
  }

  pub fn with_skip_gc(mut self, skip_gc: bool) -> Self {
    self.skip_gc = skip_gc;
    self
  }

  pub fn build(self) -> Result<Collab, CollabError> {
    let origin = CollabOrigin::Client(CollabClient::new(self.uid, self.device_id));
    let collab = Collab::new_with_source(
      origin,
      &self.object_id,
      self.source,
      self.plugins,
      self.skip_gc,
    )?;
    Ok(collab)
  }
}

#[derive(Clone)]
pub struct CollabContext {
  origin: CollabOrigin,
  doc: Doc,
  #[allow(dead_code)]
  plugins: Plugins,
  object_id: String,
}

impl CollabContext {
  fn new(origin: CollabOrigin, plugins: Plugins, doc: Doc, object_id: String) -> Self {
    Self {
      origin,
      plugins,
      doc,
      object_id,
    }
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

pub trait TransactionExt<'doc> {
  fn try_encode_state_as_update_v1(&self, sv: &StateVector) -> Result<Vec<u8>, CollabError>;
}

impl<'doc> TransactionExt<'doc> for Transaction<'doc> {
  fn try_encode_state_as_update_v1(&self, sv: &StateVector) -> Result<Vec<u8>, CollabError> {
    match panic::catch_unwind(AssertUnwindSafe(|| self.encode_state_as_update_v1(sv))) {
      Ok(update) => Ok(update),
      Err(e) => Err(CollabError::YrsEncodeStateError(format!("{:?}", e))),
    }
  }
}
// Extension trait for `TransactionMut`
pub trait TransactionMutExt<'doc> {
  /// Applies an update to the document. If the update is invalid, it will return an error.
  /// It allows to catch panics from `apply_update`.
  fn try_apply_update(&mut self, update: Update) -> Result<(), CollabError>;
  fn try_commit(&mut self) -> Result<(), CollabError>;
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

  fn try_commit(&mut self) -> Result<(), CollabError> {
    match panic::catch_unwind(AssertUnwindSafe(|| self.commit())) {
      Ok(_) => Ok(()),
      Err(e) => Err(CollabError::YrsTransactionError(format!("{:?}", e))),
    }
  }
}

fn initial_awareness_state(uid: i64) -> JsonValue {
  json!({ "uid": uid })
}
