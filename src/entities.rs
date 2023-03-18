use crate::collab::Collab;
use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::types::ToJson;
use yrs::{Doc, Map, MapRef, Transact};

pub struct MapModifier {
    doc: Doc,
    inner: MapRef,
}

impl MapModifier {
    pub fn new(map: MapRef, doc: Doc) -> Self {
        Self { doc, inner: map }
    }

    pub fn into_inner(self) -> MapRef {
        self.inner
    }

    pub fn insert<V: Prelim>(&mut self, key: &str, value: V) {
        let mut txn = self.doc.transact_mut();
        self.inner.insert(&mut txn, key, value);
        drop(txn);
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        let txn = self.doc.transact();
        self.inner.get(&txn, &key).map(|val| val.to_string(&txn))
    }

    pub fn to_json(&self) -> String {
        let txn = self.doc.transact();
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
