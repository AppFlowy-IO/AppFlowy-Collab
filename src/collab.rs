use crate::entities::MapModifier;
use crate::plugin::CollabPlugin;
use crate::util::insert_value_to_parent;
use bytes::Bytes;
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
use yrs::{
    Doc, Map, MapPrelim, MapRef, Observable, ReadTxn, Subscription, Transact, Transaction,
    TransactionMut, Update,
};

type SubscriptionCallback = Arc<dyn Fn(&TransactionMut, &MapEvent)>;
type MapSubscription = Subscription<SubscriptionCallback>;

pub struct Collab {
    id: String,
    doc: Doc,
    attributes: MapRef,
    plugins: Vec<Rc<dyn CollabPlugin>>,
    subscription: Option<MapSubscription>,
}

impl Collab {
    pub fn new(id: String, uid: i64) -> Collab {
        let doc = Doc::with_client_id(uid as u64);
        let attributes = doc.get_or_insert_map("attrs");

        Self {
            id,
            doc,
            attributes,
            plugins: vec![],
            subscription: None,
        }
    }

    pub fn observer_attrs<F>(&mut self, f: F) -> MapSubscription
    where
        F: Fn(&TransactionMut, &MapEvent) + 'static,
    {
        self.attributes.observe(f)
    }

    pub fn get_attr(&self, key: &str) -> Option<Value> {
        let txn = self.doc.transact();
        self.attributes.get(&txn, key)
    }

    pub fn insert_attr<V: Prelim>(&self, key: &str, value: V) {
        self.with_transact_mut(|txn| {
            self.attributes.insert(txn, key, value);
        })
    }

    pub fn insert_json_attr_with_path<T: Serialize>(
        &mut self,
        path: Vec<String>,
        id: &str,
        object: T,
    ) {
        let map = if path.is_empty() {
            None
        } else {
            let txn = self.transact();
            self.get_map_with_txn(&txn, path).map(|m| m.into_inner())
        };

        self.with_transact_mut(|txn| {
            let value = serde_json::to_value(&object).unwrap();
            insert_value_to_parent(id, &value, map, txn, self);
        });
    }

    pub fn get_json_attr_with_path<T: DeserializeOwned>(
        &self,
        path: impl Into<Path>,
    ) -> Option<(T, MapModifier)> {
        let path = path.into();
        if path.is_empty() {
            return None;
        }
        let txn = self.transact();
        let map = self.get_map_with_txn(&txn, path)?;
        drop(txn);

        let json_str = map.to_json();
        let object = serde_json::from_str::<T>(&json_str).ok()?;
        Some((object, map))
    }

    pub fn get_map_with_path<P: Into<Path>>(&self, path: P) -> Option<MapModifier> {
        let txn = self.doc.transact();
        self.get_map_with_txn(&txn, path)
    }

    pub fn get_map_with_txn<P: Into<Path>>(
        &self,
        txn: &Transaction,
        path: P,
    ) -> Option<MapModifier> {
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
            MapModifier::new(
                CollabTransact::new(self.plugins.clone(), self.doc.clone()),
                map_ref,
            )
        })
    }

    pub fn create_map_with_transaction(&self, id: &str, txn: &mut TransactionMut) -> MapRef {
        let map = MapPrelim::<lib0::any::Any>::new();
        self.attributes.insert(txn, id, map)
    }

    pub fn remove_attr(&mut self, key: &str) -> Option<Value> {
        let mut txn = self.doc.transact_mut();
        self.attributes.remove(&mut txn, key)
    }

    pub fn remove_attr_with_path<P: Into<Path>>(&mut self, path: P) -> Option<Value> {
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
        let transact = CollabTransact::new(self.plugins.clone(), self.doc.clone());
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
    pub fn new(id: String, uid: i64) -> Self {
        Self {
            collab: Collab::new(id, uid),
        }
    }

    pub fn from_updates(id: String, uid: i64, updates: Vec<Update>) -> Self {
        let builder = CollabBuilder::new(id, uid);
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
        self.collab.plugins.push(Rc::new(plugin));
        self
    }

    pub fn build(self) -> Collab {
        self.collab
    }
}

pub struct CollabTransact {
    plugins: Vec<Rc<dyn CollabPlugin>>,
    doc: Doc,
}

impl CollabTransact {
    pub fn new(plugins: Vec<Rc<dyn CollabPlugin>>, doc: Doc) -> Self {
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
        let state = txn.state_vector();
        let ret = f(&mut txn);

        let update = Bytes::from(txn.encode_state_as_update_v1(&state));
        self.plugins
            .iter()
            .for_each(|plugin| plugin.did_receive_new_update(update.clone()));

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
