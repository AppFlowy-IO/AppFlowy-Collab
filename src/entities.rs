use crate::collab::CollabTransact;
use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::types::ToJson;
use yrs::{Map, MapRef};

pub struct MapModifier {
    inner: MapRef,
    collab_txn: CollabTransact,
}

impl MapModifier {
    pub fn new(collab_txn: CollabTransact, map: MapRef) -> Self {
        Self {
            collab_txn,
            inner: map,
        }
    }

    pub fn into_inner(self) -> MapRef {
        self.inner
    }

    pub fn insert<V: Prelim>(&mut self, key: &str, value: V) {
        self.collab_txn.with_transact_mut(|txn| {
            self.inner.insert(txn, key, value);
        })
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        let txn = self.collab_txn.transact();
        self.inner.get(&txn, key).map(|val| val.to_string(&txn))
    }

    pub fn to_json(&self) -> String {
        let txn = self.collab_txn.transact();
        let value = self.inner.to_json(&txn);
        let mut json_str = String::new();
        value.to_json(&mut json_str);
        json_str
    }
}

impl Deref for MapModifier {
    type Target = MapRef;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for MapModifier {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
