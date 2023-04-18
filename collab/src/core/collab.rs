use crate::core::collab_plugin::CollabPlugin;
use crate::core::map_wrapper::{CustomMapRef, MapRefWrapper};
use crate::util::insert_json_value_to_map_ref;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;
use std::vec::IntoIter;
use yrs::block::Prelim;
use yrs::types::map::MapEvent;
use yrs::types::{ToJson, Value};

pub const DATA_SECTION: &str = "data";
use crate::preclude::{ArrayRefWrapper, JsonValue};

type AfterTransactionSubscription = Subscription<Arc<dyn Fn(&mut TransactionMut)>>;

use yrs::{
  ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Observable, Options, ReadTxn, Subscription,
  Transact, Transaction, TransactionMut, Update, UpdateSubscription,
};

pub type MapSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &MapEvent)>;
pub type MapSubscription = Subscription<MapSubscriptionCallback>;

pub struct Collab {
  doc: Doc,
  #[allow(dead_code)]
  object_id: String,
  data: MapRef,
  plugins: Plugins,
  #[allow(dead_code)]
  update_subscription: UpdateSubscription,
  #[allow(dead_code)]
  after_txn_subscription: AfterTransactionSubscription,
}

impl Collab {
  pub fn new<T: AsRef<str>>(uid: i64, object_id: T, plugins: Vec<Arc<dyn CollabPlugin>>) -> Collab {
    let object_id = object_id.as_ref().to_string();
    let doc = Doc::with_options(Options {
      skip_gc: true,
      client_id: uid as u64, // in order to support revisions we cannot garbage collect deleted blocks
      ..Options::default()
    });
    let data = doc.get_or_insert_map(DATA_SECTION);
    let plugins = Plugins::new(plugins);
    let (update_subscription, after_txn_subscription) =
      observe_doc(&doc, object_id.clone(), plugins.clone());
    Self {
      object_id,
      doc,
      data,
      plugins,
      update_subscription,
      after_txn_subscription,
    }
  }

  pub fn add_plugin(&mut self, plugin: Arc<dyn CollabPlugin>) {
    self.plugins.write().push(plugin);
  }

  pub fn add_plugins(&mut self, plugins: Vec<Arc<dyn CollabPlugin>>) {
    let mut write_guard = self.plugins.write();
    for plugin in plugins {
      write_guard.push(plugin);
    }
  }

  ///  
  pub fn initial(&self) {
    let mut txn = self.doc.transact_mut();
    self
      .plugins
      .read()
      .iter()
      .for_each(|plugin| plugin.init(&self.object_id, &mut txn));
    drop(txn);

    self
      .plugins
      .read()
      .iter()
      .for_each(|plugin| plugin.did_init(&self.object_id));
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
    let mut txn = self.doc.transact_mut();
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

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let transact = CollabContext::new(self.plugins.clone(), self.doc.clone());
    transact.with_transact_mut(f)
  }

  fn map_wrapper_with(&self, map_ref: MapRef) -> MapRefWrapper {
    MapRefWrapper::new(
      map_ref,
      CollabContext::new(self.plugins.clone(), self.doc.clone()),
    )
  }
  fn array_wrapper_with(&self, array_ref: ArrayRef) -> ArrayRefWrapper {
    ArrayRefWrapper::new(
      array_ref,
      CollabContext::new(self.plugins.clone(), self.doc.clone()),
    )
  }
}

fn observe_doc(
  doc: &Doc,
  oid: String,
  plugins: Plugins,
) -> (UpdateSubscription, AfterTransactionSubscription) {
  let cloned_oid = oid.clone();
  let cloned_plugins = plugins.clone();
  let update_sub = doc
    .observe_update_v1(move |txn, event| {
      cloned_plugins
        .read()
        .iter()
        .for_each(|plugin| plugin.did_receive_update(&cloned_oid, txn, &event.update));
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

pub struct CollabBuilder {
  plugins: Vec<Arc<dyn CollabPlugin>>,
  uid: i64,
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
    let mut txn = collab.doc.transact_mut();
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
  doc: Doc,
  #[allow(dead_code)]
  plugins: Plugins,
}

impl CollabContext {
  fn new(plugins: Plugins, doc: Doc) -> Self {
    Self { plugins, doc }
  }

  pub fn transact(&self) -> Transaction {
    self.doc.transact()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = self.doc.transact_mut();
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
pub struct Plugins(Rc<RwLock<Vec<Arc<dyn CollabPlugin>>>>);

impl Plugins {
  pub fn new(plugins: Vec<Arc<dyn CollabPlugin>>) -> Plugins {
    Self(Rc::new(RwLock::new(plugins)))
  }
}

impl Deref for Plugins {
  type Target = Rc<RwLock<Vec<Arc<dyn CollabPlugin>>>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
