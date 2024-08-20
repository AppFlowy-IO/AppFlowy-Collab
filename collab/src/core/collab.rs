pub use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::panic;
use std::panic::AssertUnwindSafe;

use arc_swap::ArcSwapOption;
use std::sync::Arc;
use std::vec::IntoIter;

use serde_json::json;

use tokio_stream::wrappers::WatchStream;
use yrs::block::{ClientID, Prelim};
use yrs::types::map::MapEvent;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;

use yrs::{
  Any, Doc, Map, MapRef, Observable, OffsetKind, Options, Out, ReadTxn, StateVector, Subscription,
  Transact, Transaction, TransactionMut, UndoManager, Update,
};

use crate::core::awareness::Awareness;
use crate::core::collab_plugin::{CollabPersistence, CollabPlugin, Plugins};
use crate::core::collab_state::{InitState, SnapshotState, State, SyncState};
use crate::core::origin::{CollabClient, CollabOrigin};
use crate::core::transaction::DocTransactionExtension;

use crate::entity::{EncodedCollab, EncoderVersion};
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
  state: Arc<State>,
  update_subscription: ArcSwapOption<Subscription>,
  awareness_subscription: ArcSwapOption<Subscription>,
  after_txn_subscription: ArcSwapOption<AfterTransactionSubscription>,
  /// A list of plugins that are used to extend the functionality of the [Collab].
  plugins: Plugins,
  pub index_json_sender: IndexContentSender,

  // EXPLANATION: context, meta and data are often used within the same context: &mut context
  //  used to obtain TransactionMut, which is then used by &data and &meta. This is why they are
  //  exposed as fields, instead of being accessed through methods. This way Rust borrow checker
  //  will be able to infere that &mut context and &data/&meta don't overlap.
  /// Every [Collab] instance has a data section that can be used to store
  pub data: MapRef,
  pub meta: MapRef,
  /// This is an inner collab state that requires mut access in order to modify it.
  pub context: CollabContext,
}

unsafe impl Send for Collab {} // TODO: Remove this once MapRefs are Send
unsafe impl Sync for Collab {} // TODO: Remove this once MapRefs are Sync

pub struct CollabContext {
  /// This [CollabClient] is used to verify the origin of a [LockedTransaction] when
  /// applying a remote update.
  origin: CollabOrigin,
  /// The [Awareness] is used to track the awareness of the other peers.
  awareness: Awareness,
  /// The [UndoManager] is used to undo and redo changes. By default, the [UndoManager]
  /// is disabled. To enable it, call [Collab::enable_undo_manager].
  undo_manager: Option<UndoManager>,

  /// The current transaction that is being executed.
  current_txn: Option<TransactionMut<'static>>,
}

unsafe impl Send for CollabContext {}
unsafe impl Sync for CollabContext {}

impl CollabContext {
  fn new(origin: CollabOrigin, awareness: Awareness) -> Self {
    CollabContext {
      origin,
      awareness,
      undo_manager: None,
      current_txn: None,
    }
  }

  pub fn with_txn<F, T>(&mut self, f: F) -> Result<T, CollabError>
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut cleanup = false;
    if self.current_txn.is_none() {
      let txn: TransactionMut<'_> = self.transact_mut();
      self.current_txn = Some(unsafe {
        std::mem::transmute::<yrs::TransactionMut<'_>, yrs::TransactionMut<'static>>(txn)
      });
      cleanup = true;
    }

    let txn = self.current_txn.as_mut().unwrap();

    // if we let panics happen, we might not be able to cleanup broken transaction
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| f(txn)))
      .map_err(|_| CollabError::YrsTransactionError("failed to execute transaction".to_string()));

    if cleanup {
      // the call which initialized the transaction is responsible for cleaning it up
      self.current_txn = None;
    }
    result
  }

  #[inline]
  pub(crate) fn doc(&self) -> &Doc {
    self.awareness.doc()
  }

  //TODO: fix naming convention (by Rust standards it should be `awareness`)
  #[inline]
  pub fn get_awareness(&self) -> &Awareness {
    &self.awareness
  }

  //TODO: fix naming convention (by Rust standards it should be `awareness_mut`)
  #[inline]
  pub fn get_mut_awareness(&mut self) -> &mut Awareness {
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

  pub fn transact_mut(&mut self) -> TransactionMut {
    self.doc().transact_mut_with(self.origin.clone())
  }

  pub fn undo(&mut self) -> Result<bool, CollabError> {
    let undo_manager = self.undo_manager_mut()?;
    Ok(undo_manager.undo()?)
  }

  pub fn redo(&mut self) -> Result<bool, CollabError> {
    let undo_manager = self.undo_manager_mut()?;
    Ok(undo_manager.redo()?)
  }

  pub fn apply_update(&mut self, update: Update) -> Result<(), CollabError> {
    self.with_txn(|tx| tx.apply_update(update))
  }

  pub fn clean_awareness_state(&mut self) {
    self.awareness.clean_local_state();
  }

  pub fn emit_awareness_state(&mut self) {
    let state = if let CollabOrigin::Client(origin) = &self.origin {
      Some(initial_awareness_state(origin.uid))
    } else {
      None
    };
    if let Some(state) = state {
      if let Err(e) = self.awareness.set_local_state(state) {
        tracing::warn!("Failed to set awareness state: {}", e);
      }
    }
  }

  pub fn client_id(&self) -> ClientID {
    self.doc().client_id()
  }

  pub fn transact(&self) -> Transaction {
    self.doc().transact()
  }

  pub fn can_undo(&self) -> bool {
    match self.undo_manager() {
      Ok(mgr) => mgr.can_undo(),
      Err(_) => false,
    }
  }

  pub fn can_redo(&self) -> bool {
    match self.undo_manager() {
      Ok(mgr) => mgr.can_redo(),
      Err(_) => false,
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
    data_source: DataSource,
    plugins: Vec<Box<dyn CollabPlugin>>,
    skip_gc: bool,
  ) -> Result<Self, CollabError> {
    let mut collab = Self::new_with_origin(origin, object_id, plugins, skip_gc);
    match data_source {
      DataSource::Disk(disk) => {
        if let Some(disk) = disk {
          disk.load_collab(&mut collab);
        }
      },
      DataSource::DocStateV1(doc_state) => {
        if !doc_state.is_empty() {
          let update = Update::decode_v1(&doc_state)?;
          collab.context.apply_update(update)?;
        }
      },
      DataSource::DocStateV2(doc_state) => {
        if !doc_state.is_empty() {
          let update = Update::decode_v2(&doc_state)?;
          collab.context.apply_update(update)?;
        }
      },
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
    let plugins = Plugins::new(plugins);
    let state = Arc::new(State::new(&object_id));
    let awareness = Awareness::new(doc);
    Self {
      object_id,
      context: CollabContext::new(origin, awareness),
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

  pub fn object_id(&self) -> &str {
    &self.object_id
  }

  pub fn origin(&self) -> &CollabOrigin {
    &self.context.origin
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

    let doc = self.context.doc();
    self.state.set_init_state(InitState::Loading);
    {
      let origin = self.origin();
      self
        .plugins
        .each(|plugin| plugin.init(&self.object_id, origin, doc));
    }

    let (update_subscription, after_txn_subscription) = observe_doc(
      doc,
      self.object_id.clone(),
      self.plugins.clone(),
      self.origin().clone(),
    );

    let awareness_subscription = observe_awareness(
      self.context.get_awareness(),
      self.plugins.clone(),
      self.object_id.clone(),
      self.origin().clone(),
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

  pub fn get_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<Out> {
    self.data.get(txn, key)
  }

  pub fn start_init_sync(&self) {
    self.plugins.each(|plugin| {
      plugin.start_init_sync();
    });
  }

  pub fn insert<P>(&mut self, key: &str, value: P) -> P::Return
  where
    P: Prelim,
  {
    self
      .context
      .with_txn(|tx| self.data.insert(tx, key, value))
      .unwrap()
  }

  /// Make a full update with the current state of the [Collab].
  /// It invokes the [CollabPlugin::flush] method of each plugin.
  pub fn flush(&self) {
    self
      .plugins
      .each(|plugin| plugin.write_to_disk(&self.object_id))
  }

  pub fn get<V>(&self, key: &str) -> Option<V>
  where
    V: TryFrom<Out, Error = Out>,
  {
    let tx = self.context.transact();
    let value = self.data.get(&tx, key)?;
    V::try_from(value).ok()
  }

  pub fn remove(&mut self, key: &str) -> Option<Out> {
    self
      .context
      .with_txn(|tx| self.data.remove(tx, key))
      .unwrap()
  }

  pub fn enable_undo_redo(&mut self) {
    if self.context.undo_manager.is_some() {
      tracing::warn!("Undo manager already enabled");
      return;
    }
    // a frequent case includes establishing a new transaction for every user key stroke. Meanwhile
    // we may decide to use different granularity of undo/redo actions. These are grouped together
    // on time-based ranges (configurable in undo::Options, which is 500ms by default).
    let mut undo_manager = UndoManager::with_scope_and_options(
      self.context.doc(),
      &self.data,
      yrs::undo::Options::default(),
    );
    undo_manager.include_origin(self.origin().clone());
    self.context.undo_manager = Some(undo_manager);
  }

  /// Returns the doc state and the state vector.
  pub fn encode_collab_v1<F, E>(&self, validate: F) -> Result<EncodedCollab, E>
  where
    F: FnOnce(&Collab) -> Result<(), E>,
    E: std::fmt::Debug,
  {
    validate(self)?;
    let tx = self.context.transact();
    Ok(tx.get_encoded_collab_v1())
  }

  pub fn encode_collab_v2(&self) -> EncodedCollab {
    let tx = self.context.transact();
    tx.get_encoded_collab_v2()
  }

  pub fn get_last_sync_at(&self) -> i64 {
    let txn = self.context.transact();
    self
      .meta
      .get(&txn, LAST_SYNC_AT)
      .and_then(|v| v.cast().ok())
      .unwrap_or(0)
  }

  pub fn set_last_sync_at(&mut self, last_sync_at: i64) {
    //FIXME: this is very expensive to do on frequent basis. That's one of the reasons we have
    // awareness state separate from document
    let mut txn = self.context.transact_mut();
    self
      .meta
      .insert(&mut txn, LAST_SYNC_AT, Any::BigInt(last_sync_at));
  }

  pub fn to_json(&self) -> Any {
    self.data.to_json(&self.context.transact())
  }

  pub fn to_json_value(&self) -> JsonValue {
    serde_json::to_value(self.data.to_json(&self.context.transact())).unwrap()
  }
}

impl Deref for Collab {
  type Target = CollabContext;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.context
  }
}

impl DerefMut for Collab {
  #[inline]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.context
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
      plugins.each(|plugin| plugin.receive_local_state(&origin, &oid, e, &update));
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
  Disk(Option<Box<dyn CollabPersistence>>),
  DocStateV1(Vec<u8>),
  DocStateV2(Vec<u8>),
}

impl From<EncodedCollab> for DataSource {
  fn from(encoded: EncodedCollab) -> Self {
    match encoded.version {
      EncoderVersion::V1 => DataSource::DocStateV1(encoded.doc_state.into()),
      EncoderVersion::V2 => DataSource::DocStateV2(encoded.doc_state.into()),
    }
  }
}

impl DataSource {
  pub fn is_empty(&self) -> bool {
    matches!(self, DataSource::Disk(_))
  }

  pub fn as_update(&self) -> Result<Option<Update>, CollabError> {
    match self {
      DataSource::DocStateV1(doc_state) if !doc_state.is_empty() => {
        Ok(Some(Update::decode_v1(doc_state)?))
      },
      DataSource::DocStateV2(doc_state) if !doc_state.is_empty() => {
        Ok(Some(Update::decode_v2(doc_state)?))
      },
      _ => Ok(None),
    }
  }
}
impl CollabBuilder {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T, data_source: DataSource) -> Self {
    let object_id = object_id.as_ref();
    Self {
      uid,
      plugins: vec![],
      object_id: object_id.to_string(),
      device_id: "".to_string(),
      source: data_source,
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
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
      self.apply_update(update);
    }));
    match result {
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
