use crate::collab::CollabTransact;
use crate::insert_json_value_to_map_ref;
use crate::util::lib0_any_to_json_value;
use lib0::any::Any;
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::types::{ToJson, Value};
use yrs::{Map, MapRef, ReadTxn, Transaction, TransactionMut};

pub trait CustomMapRef {
    fn from_map_ref(map_ref: MapRefWrapper) -> Self;
}

impl CustomMapRef for MapRefWrapper {
    fn from_map_ref(map_ref: MapRefWrapper) -> Self {
        map_ref
    }
}

pub struct MapRefWrapper {
    map_ref: MapRef,
    collab_txn: CollabTransact,
}

impl MapRefWrapper {
    pub fn new(map_ref: MapRef, collab_txn: CollabTransact) -> Self {
        Self {
            collab_txn,
            map_ref,
        }
    }

    pub fn into_inner(self) -> MapRef {
        self.map_ref
    }

    pub fn insert<V: Prelim>(&mut self, key: &str, value: V) {
        self.collab_txn.with_transact_mut(|txn| {
            self.map_ref.insert(txn, key, value);
        })
    }

    pub fn insert_with_txn<V: Prelim>(&mut self, txn: &mut TransactionMut, key: &str, value: V) {
        self.map_ref.insert(txn, key, value);
    }

    pub fn insert_json<T: Serialize>(&mut self, key: &str, value: T) {
        let value = serde_json::to_value(&value).unwrap();
        self.collab_txn.with_transact_mut(|txn| {
            insert_json_value_to_map_ref(key, &value, self.map_ref.clone(), txn);
        });
    }

    pub fn insert_json_with_txn<T: Serialize>(
        &mut self,
        txn: &mut TransactionMut,
        key: &str,
        value: T,
    ) {
        let value = serde_json::to_value(&value).unwrap();
        if let Some(map_ref) = self.get_map_with_txn(txn, key) {
            insert_json_value_to_map_ref(key, &value, map_ref, txn);
        }
    }

    pub fn get_map_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<MapRef> {
        if let Some(Value::YMap(map_ref)) = self.map_ref.get(txn, key) {
            return Some(map_ref);
        }
        None
    }

    pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get_json_with_txn(&self.collab_txn.transact(), key)
    }

    pub fn get_json_with_txn<T: DeserializeOwned>(
        &self,
        txn: &Transaction,
        key: &str,
    ) -> Option<T> {
        let map_ref = self.get_map_with_txn(txn, key)?;
        let json_value = lib0_any_to_json_value(map_ref.to_json(txn)).ok()?;
        serde_json::from_value::<T>(json_value).ok()
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        let txn = self.collab_txn.transact();
        self.get_str_with_txn(&txn, key)
    }

    pub fn get_str_with_txn(&self, txn: &Transaction, key: &str) -> Option<String> {
        if let Some(Value::Any(Any::String(value))) = self.map_ref.get(txn, key) {
            return Some(value.to_string());
        }
        None
    }

    pub fn get_i64_with_txn(&self, txn: &Transaction, key: &str) -> Option<i64> {
        if let Some(Value::Any(Any::BigInt(value))) = self.map_ref.get(txn, key) {
            return Some(value);
        }
        None
    }

    pub fn get_f64_with_txn(&self, txn: &Transaction, key: &str) -> Option<f64> {
        if let Some(Value::Any(Any::Number(value))) = self.map_ref.get(txn, key) {
            return Some(value);
        }
        None
    }

    pub fn get_bool_with_txn(&self, txn: &Transaction, key: &str) -> Option<bool> {
        if let Some(Value::Any(Any::Bool(value))) = self.map_ref.get(txn, key) {
            return Some(value);
        }
        None
    }

    pub fn to_json(&self) -> String {
        let txn = self.collab_txn.transact();
        let value = self.map_ref.to_json(&txn);
        let mut json_str = String::new();
        value.to_json(&mut json_str);
        json_str
    }
}

impl Deref for MapRefWrapper {
    type Target = MapRef;

    fn deref(&self) -> &Self::Target {
        &self.map_ref
    }
}

impl DerefMut for MapRefWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map_ref
    }
}
