pub use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::panic;
use std::panic::AssertUnwindSafe;

use arc_swap::ArcSwapOption;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::vec::IntoIter;

use serde_json::json;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use tokio_stream::wrappers::WatchStream;
use yrs::block::{ClientID, Prelim};
use yrs::types::map::MapEvent;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;

use yrs::{
  Any, ArrayPrelim, ArrayRef, Doc, In, Map, MapPrelim, MapRef, Observable, OffsetKind, Options,
  Origin, Out, ReadTxn, StateVector, Subscription, Transact, Transaction, TransactionMut,
  UndoManager, Update,
};

use crate::core::awareness::Awareness;
use crate::core::collab_plugin::{CollabPlugin, Plugins};
use crate::core::collab_state::{InitState, SnapshotState, State, SyncState};
use crate::core::origin::{CollabClient, CollabOrigin};
use crate::core::transaction::DocTransactionExtension;
use crate::core::value::Entity;
use crate::entity::EncodedCollab;
use crate::error::CollabError;
use crate::preclude::JsonValue;

pub const DATA_SECTION: &str = "data";
pub const META_SECTION: &str = "meta";

const LAST_SYNC_AT: &str = "last_sync_at";

type AfterTransactionSubscription = Subscription;

pub type MapSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &MapEvent)>;
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
  /// This [CollabClient] is used to verify the origin of a [LockedTransaction] when
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
  /// This is an inner collab state that requires mut access in order to modify it.
  inner: RwLock<CollabData>,
}

unsafe impl Send for Collab {} // TODO: Remove this once MapRefs are Send
unsafe impl Sync for Collab {} // TODO: Remove this once MapRefs are Sync

pub struct CollabData {
  /// The [Awareness] is used to track the awareness of the other peers.
  pub awareness: Awareness,
  /// The [UndoManager] is used to undo and redo changes. By default, the [UndoManager]
  /// is disabled. To enable it, call [Collab::enable_undo_manager].
  pub undo_manager: Option<UndoManager>,
}

impl CollabData {
  pub(crate) fn doc(&self) -> &Doc {
    self.awareness.doc()
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
    let update = collab_doc_state.as_update()?;
    if let Some(update) = update {
      let lock = collab.inner.try_write().unwrap();
      let mut txn = lock.doc().transact_mut_with(collab.origin.clone());
      txn.apply_update(update);
    }
    Ok(collab)
  }

  pub fn clear_plugins(&self) {
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
    let awareness = Awareness::new(doc);
    Self {
      origin,
      object_id,
      inner: RwLock::new(CollabData {
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

  #[inline]
  pub async fn read(&self) -> OwnedCollab<CollabRead> {
    OwnedCollab::acquire_read(self).await
  }

  #[inline]
  pub fn blocking_read(&self) -> OwnedCollab<CollabRead> {
    OwnedCollab::blocking_acquire_read(self)
  }

  #[inline]
  pub async fn write(&self) -> OwnedCollab<CollabWrite> {
    OwnedCollab::acquire_write(self).await
  }

  #[inline]
  pub fn blocking_write(&self) -> OwnedCollab<CollabWrite> {
    OwnedCollab::blocking_acquire_write(self)
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

  /// Subscribes to the `IndexJson` associated with a `Collab` object.
  ///
  /// `IndexJson` is a JSON object containing data used for indexing purposes. The structure and
  /// content of this data may vary between different collaborative objects derived from `Collab`.
  /// The interpretation of `IndexJson` is specific to the subscriber, as only they know how to
  /// process and utilize the contained indexing information.
  pub fn subscribe_index_content(&self) -> IndexContentReceiver {
    self.index_json_sender.subscribe()
  }

  /// Add a plugin to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugin(&self, plugin: Box<dyn CollabPlugin>) {
    self.add_plugins([plugin]);
  }

  /// Add plugins to the [Collab]. The plugin's callbacks will be called in the order they are added.
  pub fn add_plugins<I>(&self, plugins: I)
  where
    I: IntoIterator<Item = Box<dyn CollabPlugin>>,
  {
    for plugin in plugins.into_iter() {
      if !self.plugins.push_front(plugin) {
        tracing::error!("Only one cloud storage plugin can be added to a collab instance.");
      }
    }
  }

  pub fn set_last_sync_at_with_txn(&self, txn: &mut TransactionMut, last_sync_at: i64) {
    //FIXME: this is very expensive to do on frequent basis. That's one of the reasons we have
    // awareness state separate from document
    self.meta.insert(txn, LAST_SYNC_AT, last_sync_at);
  }

  pub fn set_sync_state(&self, sync_state: SyncState) {
    self.state.set_sync_state(sync_state);
  }

  pub fn set_snapshot_state(&self, snapshot_state: SnapshotState) {
    self.state.set_snapshot_state(snapshot_state);
  }

  pub fn observe_data<F>(&self, f: F) -> MapSubscription
  where
    F: Fn(&TransactionMut, &MapEvent) + Send + Sync + 'static,
  {
    self.data.observe(f)
  }

  pub fn get_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<Value> {
    self.data.get(txn, key)
  }

  pub fn insert_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    value: V,
  ) -> V::Return {
    self.data.insert(txn, key, value)
  }

  #[inline]
  pub fn get_with_path<P, T, V>(&self, txn: &T, path: P) -> Option<V>
  where
    P: Into<Path>,
    T: ReadTxn,
    V: TryFrom<Out, Error = Out>,
  {
    let value = self.get_value_with_path(txn, path)?;
    value.cast::<V>().ok()
  }

  pub fn get_value_with_path<P, T>(&self, txn: &T, path: P) -> Option<Value>
  where
    P: Into<Path>,
    T: ReadTxn,
  {
    let mut current = self.data.clone();
    let mut path = path.into();
    let last = path.pop()?;
    for field in path {
      current = current.get(txn, &field)?.cast().ok()?;
    }
    current.get(txn, &last)
  }

  pub fn insert_json_with_path<P, V>(
    &self,
    txn: &mut TransactionMut,
    path: P,
    value: V,
  ) -> Result<(), CollabError>
  where
    P: Into<Path>,
    V: Serialize,
  {
    let value = serde_json::to_value(value)?;
    self.insert_with_path(txn, path, Entity::from(value))?;
    Ok(())
  }

  pub fn get_json_with_path<T, P, V>(&self, txn: &T, path: P) -> Result<V, CollabError>
  where
    T: ReadTxn,
    P: Into<Path>,
    V: DeserializeOwned,
  {
    let value = self
      .get_value_with_path(txn, path)
      .ok_or(CollabError::UnexpectedEmpty(
        "value not found on path".to_string(),
      ))?;
    let value = serde_json::to_value(value.to_json(txn))?;
    Ok(serde_json::from_value(value)?)
  }

  pub fn insert_with_path<P, V>(
    &self,
    txn: &mut TransactionMut,
    path: P,
    value: V,
  ) -> Result<V::Return, CollabError>
  where
    P: Into<Path>,
    V: Prelim,
  {
    let mut current = self.data.clone();
    let mut path = path.into();
    let last = match path.pop() {
      Some(field) => field,
      None => return Err(CollabError::NoRequiredData("empty path".into())),
    };
    for field in path {
      current = match current.get(txn, &field) {
        None => current.insert(txn, field, MapPrelim::<Any>::new()),
        Some(value) => value
          .cast()
          .map_err(|_| CollabError::NoRequiredData(field))?,
      };
    }
    Ok(current.insert(txn, last, value))
  }

  pub fn remove_with_path<P>(&mut self, txn: &mut TransactionMut<'_>, path: P) -> Option<Out>
  where
    P: Into<Path>,
  {
    let mut path = path.into();
    if path.is_empty() {
      return None;
    }
    let last = path.pop()?;
    let mut current = self.data.clone();
    for field in path {
      current = current.get(txn, &field)?.cast().ok()?;
    }
    current.remove(txn, &last)
  }

  pub fn start_init_sync(&self) {
    self.plugins.each(|plugin| {
      plugin.start_init_sync();
    });
  }
}

pub type CollabRead<'a> = RwLockReadGuard<'a, CollabData>;
pub type CollabWrite<'a> = RwLockWriteGuard<'a, CollabData>;

pub struct OwnedCollab<'a, L> {
  collab: &'a Collab,
  lock: L,
}

impl<'a> OwnedCollab<'a, CollabRead<'a>> {
  #[inline]
  pub async fn acquire_read(collab: &'a Collab) -> Self {
    let lock = collab.inner.read().await;
    Self { collab, lock }
  }

  #[inline]
  pub fn blocking_acquire_read(collab: &'a Collab) -> Self {
    let lock = collab.inner.blocking_read();
    Self { collab, lock }
  }
}

impl<'a> OwnedCollab<'a, CollabWrite<'a>> {
  #[inline]
  pub async fn acquire_write(collab: &'a Collab) -> Self {
    let lock = collab.inner.write().await;
    Self { collab, lock }
  }

  #[inline]
  pub fn blocking_acquire_write(collab: &'a Collab) -> Self {
    let lock = collab.inner.blocking_write();
    Self { collab, lock }
  }

  #[inline]
  pub fn get_mut_awareness(&mut self) -> &mut Awareness {
    //FIXME: naming convention should be `awareness_mut`
    self.lock.awareness_mut()
  }

  pub fn transact_mut(&self) -> TransactionMut {
    self
      .lock
      .doc()
      .transact_mut_with(self.collab.origin.clone())
  }

  pub fn set_last_sync_at(&self, last_sync_at: i64) {
    let mut txn = self.transact_mut();
    self.set_last_sync_at_with_txn(&mut txn, last_sync_at)
  }

  pub fn undo(&mut self) -> Result<bool, CollabError> {
    let undo_manager = self
      .lock
      .undo_manager
      .as_mut()
      .ok_or(CollabError::UndoManagerNotEnabled)?;
    Ok(undo_manager.undo()?)
  }

  pub fn redo(&mut self) -> Result<bool, CollabError> {
    let undo_manager = self
      .lock
      .undo_manager
      .as_mut()
      .ok_or(CollabError::UndoManagerNotEnabled)?;
    Ok(undo_manager.redo()?)
  }

  pub fn apply_update(&mut self, update: Update) -> Result<(), CollabError> {
    let mut tx = self.transact_mut();
    std::panic::catch_unwind(AssertUnwindSafe(|| tx.apply_update(update)))
      .map_err(|e| CollabError::YrsTransactionError(format!("{:?}", e)))
  }

  pub fn clean_awareness_state(&mut self) {
    self.lock.awareness.clean_local_state();
  }

  pub fn emit_awareness_state(&mut self) {
    let state = if let CollabOrigin::Client(origin) = &self.origin {
      Some(initial_awareness_state(origin.uid).to_string())
    } else {
      None
    };
    if let Some(state) = state {
      self.lock.awareness.set_local_state(state);
    }
  }

  /// Upon calling this method, the [Collab]'s document will be initialized with the plugins. The callbacks from the plugins
  /// will be triggered in the order they were added. The input parameter, [init_sync], indicates whether the
  /// [Collab] is initialized with local data or remote updates. If true, it suggests that the data doesn't need
  /// further synchronization with the remote server.
  ///
  /// This method must be called after all plugins have been added.
  pub fn initialize(&mut self) {
    if !self.state.is_uninitialized() {
      return;
    }

    let doc = self.lock.doc();
    self.state.set_init_state(InitState::Loading);
    {
      self
        .plugins
        .each(|plugin| plugin.init(&self.object_id, &self.origin, doc));
    }

    let (update_subscription, after_txn_subscription) = observe_doc(
      doc,
      self.object_id.clone(),
      self.plugins.clone(),
      self.origin.clone(),
    );

    let awareness_subscription = observe_awareness(
      self.lock.awareness(),
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
        .each(|plugin| plugin.did_init(self, &self.object_id, last_sync_at));
    }
    self.state.set_init_state(InitState::Initialized);
  }

  pub fn insert<P>(&mut self, key: &str, value: P) -> P::Return
  where
    P: Prelim,
  {
    let mut tx = self.transact_mut();
    self.data.insert(&mut tx, key, value)
  }

  /// Make a full update with the current state of the [Collab].
  /// It invokes the [CollabPlugin::flush] method of each plugin.
  pub fn flush(&self) {
    let doc = self.lock.doc();
    self
      .plugins
      .each(|plugin| plugin.flush(&self.object_id, doc));
  }

  pub fn remove(&mut self, key: &str) -> Option<Value> {
    let mut txn = self.lock.doc().transact_mut_with(self.origin.clone());
    self.data.remove(&mut txn, key)
  }

  pub fn enable_undo_redo(&mut self) {
    if self.lock.undo_manager.is_some() {
      tracing::warn!("Undo manager already enabled");
      return;
    }
    // a frequent case includes establishing a new transaction for every user key stroke. Meanwhile
    // we may decide to use different granularity of undo/redo actions. These are grouped together
    // on time-based ranges (configurable in undo::Options, which is 500ms by default).
    let mut undo_manager =
      UndoManager::with_scope_and_options(self.inner.doc(), &self.data, yrs::undo::Options::default());
    undo_manager.include_origin(self.origin.clone());
    self.lock.undo_manager = Some(undo_manager);
  }
}

impl<'a, L> Deref for OwnedCollab<'a, L> {
  type Target = Collab;

  #[inline]
  fn deref(&self) -> &Self::Target {
    self.collab
  }
}

pub trait CollabReadOps<'a>: Deref<Target = Collab> {
  fn collab_state(&self) -> &CollabData;

  fn client_id(&self) -> ClientID {
    self.collab_state().doc().client_id()
  }

  fn transact(&self) -> Transaction {
    self.collab_state().doc().transact()
  }

  fn get_awareness(&self) -> &Awareness {
    //FIXME: naming convention should be `awareness`
    self.collab_state().awareness()
  }

  fn can_undo(&self) -> bool {
    let lock = self.collab_state();
    match lock.undo_manager() {
      Ok(undo_manager) => undo_manager.can_undo(),
      Err(_) => false,
    }
  }

  fn can_redo(&self) -> bool {
    let lock = self.collab_state();
    match lock.undo_manager() {
      Ok(undo_manager) => undo_manager.can_redo(),
      Err(_) => false,
    }
  }

  fn get<V>(&self, key: &str) -> Option<V>
  where
    V: TryFrom<Value, Error = Value>,
  {
    let tx = self.transact();
    let value = self.data.get(&tx, key)?;
    V::try_from(value).ok()
  }

  /// Returns the doc state and the state vector.
  fn encode_collab_v1<F, E>(&self, validate: F) -> Result<EncodedCollab, E>
  where
    F: FnOnce(&Collab) -> Result<(), E>,
    E: std::fmt::Debug,
  {
    validate(self.deref())?;
    let tx = self.transact();
    Ok(tx.get_encoded_collab_v1())
  }

  fn encode_collab_v2(&self) -> EncodedCollab {
    let tx = self.transact();
    tx.get_encoded_collab_v2()
  }

  fn get_last_sync_at(&mut self) -> i64 {
    let txn = self.transact();
    self
      .meta
      .get(&txn, LAST_SYNC_AT)
      .and_then(|v| v.cast().ok())
      .unwrap_or(0)
  }

  fn to_json(&self) -> Any {
    self.data.to_json(&self.transact())
  }

  fn to_json_value(&self) -> JsonValue {
    serde_json::to_value(&self.data.to_json(&self.transact())).unwrap()
  }
}

impl<'a> CollabReadOps<'a> for OwnedCollab<'a, CollabRead<'a>> {
  #[inline]
  fn collab_state(&self) -> &CollabData {
    self.lock.deref()
  }
}

impl<'a> CollabReadOps<'a> for OwnedCollab<'a, CollabWrite<'a>> {
  #[inline]
  fn collab_state(&self) -> &CollabData {
    self.lock.deref()
  }
}

fn observe_awareness(
  awareness: &Awareness,
  plugins: Plugins,
  oid: String,
  origin: CollabOrigin,
) -> Subscription {
  awareness.on_update(move |awareness, e, _| {
    if let Ok(update) = awareness.update_with_clients(e.all_changes()) {
      plugins
        .each(|plugin| plugin.receive_local_state(&origin, &oid, e, &update));
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
      cloned_plugins.each(|plugin| {
        plugin.receive_update(&cloned_oid, txn, &event.update);

        let remote_origin = CollabOrigin::from(txn);
        if remote_origin == local_origin {
          plugin.receive_local_update(&local_origin, &cloned_oid, &event.update);
        } else {
          #[cfg(feature = "verbose_log")]
          tracing::trace!("{} did apply remote {} update", local_origin, remote_origin);
        }
      });
    })
    .unwrap();

  let after_txn_sub = doc
    .observe_after_transaction(move |txn| {
      plugins.each(|plugin| plugin.after_transaction(&oid, txn))
    })
    .ok();

  (update_sub, after_txn_sub)
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

  pub fn as_update(&self) -> Result<Option<Update>, CollabError> {
    match self {
      DataSource::DocStateV1(doc_state) if !doc_state.is_empty() => {
        Ok(Some(Update::decode_v1(&doc_state)?))
      },
      DataSource::DocStateV2(doc_state) if !doc_state.is_empty() => {
        Ok(Some(Update::decode_v2(&doc_state)?))
      },
      _ => Ok(None),
    }
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
pub struct Path(Vec<String>);

impl IntoIterator for Path {
  type Item = String;
  type IntoIter = IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

impl<const N: usize> From<[&'static str; N]> for Path {
  fn from(value: [&'static str; N]) -> Self {
    Path(value.into_iter().map(|value| value.to_string()).collect())
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
