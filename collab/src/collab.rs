use crate::collab_plugin::CollabPlugin;
use crate::map_wrapper::{CustomMapRef, MapRefWrapper};
use crate::util::insert_json_value_to_map_ref;
use bytes::Bytes;
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
use yrs::updates::encoder::Encode;
use yrs::{
    Doc, Map, MapPrelim, MapRef, Observable, ReadTxn, Subscription, Transact, Transaction,
    TransactionMut, Update, UpdateSubscription,
};

type SubscriptionCallback = Arc<dyn Fn(&TransactionMut, &MapEvent)>;
type MapSubscription = Subscription<SubscriptionCallback>;

pub struct Collab {
    doc: Doc,
    attributes: MapRef,
    plugins: Plugins,
    subscription: UpdateSubscription,
}

impl Collab {
    pub fn new(uid: i64) -> Collab {
        let doc = Doc::with_client_id(uid as u64);
        let attributes = doc.get_or_insert_map("attrs");
        let plugins = Plugins::new();

        let cloned_plugins = plugins.clone();
        let subscription = doc
            .observe_update_v1(move |txn, event| {
                cloned_plugins
                    .read()
                    .iter()
                    .for_each(|plugin| plugin.did_receive_update(txn, &event.update));
            })
            .unwrap();

        Self {
            doc,
            attributes,
            plugins,
            subscription,
        }
    }

    pub fn observer_attrs<F>(&mut self, f: F) -> MapSubscription
    where
        F: Fn(&TransactionMut, &MapEvent) + 'static,
    {
        self.attributes.observe(f)
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let txn = self.doc.transact();
        self.attributes.get(&txn, key)
    }

    pub fn insert<V: Prelim>(&self, key: &str, value: V) {
        self.with_transact_mut(|txn| self.insert_with_txn(txn, key, value))
    }

    pub fn insert_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, key: &str, value: V) {
        self.attributes.insert(txn, key, value);
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
                map = Some(self.create_map_with_transaction(key, txn));
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

    pub fn get_map_with_path<M: CustomMapRef>(&self, path: impl Into<Path>) -> Option<M> {
        let txn = self.doc.transact();
        let map_ref = self.get_map_with_txn(&txn, path)?;
        Some(M::from_map_ref(map_ref))
    }

    pub fn get_map_with_txn<P: Into<Path>>(
        &self,
        txn: &Transaction,
        path: P,
    ) -> Option<MapRefWrapper> {
        let path = path.into();
        if path.is_empty() {
            return None;
        }
        let mut iter = path.into_iter();
        let mut map_ref = self.attributes.get(txn, &iter.next().unwrap())?.to_ymap();
        for path in iter {
            map_ref = map_ref?.get(txn, &path)?.to_ymap();
        }
        map_ref.map(|map_ref| {
            MapRefWrapper::new(
                map_ref,
                CollabContext::new(self.plugins.clone(), self.doc.clone()),
            )
        })
    }

    pub fn create_map_with_transaction(&self, key: &str, txn: &mut TransactionMut) -> MapRef {
        let map = MapPrelim::<lib0::any::Any>::new();
        self.attributes.insert(txn, key, map)
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let mut txn = self.doc.transact_mut();
        self.attributes.remove(&mut txn, key)
    }

    pub fn remove_with_path<P: Into<Path>>(&mut self, path: P) -> Option<Value> {
        let path = path.into();
        if path.is_empty() {
            return None;
        }
        let len = path.len();
        if len == 1 {
            self.with_transact_mut(|txn| self.attributes.remove(txn, &path[0]))
        } else {
            let txn = self.transact();
            let mut iter = path.into_iter();
            let mut remove_path = iter.next().unwrap();
            let mut map_ref = self.attributes.get(&txn, &remove_path)?.to_ymap();

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
        self.attributes.to_json(&txn)
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
}

impl Display for Collab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())?;
        Ok(())
    }
}

pub struct CollabBuilder {
    collab: Collab,
}

impl CollabBuilder {
    pub fn new(uid: i64) -> Self {
        Self {
            collab: Collab::new(uid),
        }
    }

    pub fn from_updates(uid: i64, updates: Vec<Update>) -> Self {
        let builder = CollabBuilder::new(uid);
        let mut txn = builder.collab.doc.transact_mut();
        for update in updates {
            txn.apply_update(update);
        }
        drop(txn);
        builder
    }

    pub fn with_plugin<T>(mut self, plugin: T) -> Self
    where
        T: CollabPlugin + 'static,
    {
        self.collab.plugins.push(plugin);
        self
    }

    pub fn build(self) -> Collab {
        self.collab
    }
}

pub struct CollabContext {
    doc: Doc,
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
pub(crate) struct Plugins(Rc<RwLock<Vec<Box<dyn CollabPlugin>>>>);

impl Plugins {
    pub fn new() -> Plugins {
        Self::default()
    }

    pub fn push<P>(&self, plugin: P)
    where
        P: CollabPlugin + 'static,
    {
        self.0.write().push(Box::new(plugin));
    }
}

impl Deref for Plugins {
    type Target = Rc<RwLock<Vec<Box<dyn CollabPlugin>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
