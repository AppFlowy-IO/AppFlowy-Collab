use crate::util::lib0_any_to_json_value;
use lib0::any::Any;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::text_wrapper::TextRefWrapper;
use crate::preclude::*;
use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::types::{Delta, ToJson, Value};
use yrs::{Map, MapPrelim, MapRef, ReadTxn, TextPrelim, TextRef, Transaction, TransactionMut};

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
    collab_ctx: CollabContext,
}

impl MapRefWrapper {
    pub fn new(map_ref: MapRef, collab_ctx: CollabContext) -> Self {
        Self {
            collab_ctx,
            map_ref,
        }
    }

    pub fn into_inner(self) -> MapRef {
        self.map_ref
    }

    pub fn insert<V: Prelim>(&self, key: &str, value: V) {
        self.collab_ctx.with_transact_mut(|txn| {
            self.map_ref.insert(txn, key, value);
        })
    }

    pub fn insert_text_with_txn(&self, txn: &mut TransactionMut, key: &str) -> TextRefWrapper {
        let text = TextPrelim::new("");
        let text_ref = self.map_ref.insert(txn, key, text);
        TextRefWrapper::new(text_ref, self.collab_ctx.clone())
    }

    pub fn insert_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, key: &str, value: V) {
        self.map_ref.insert(txn, key, value);
    }

    pub fn insert_json<T: Serialize>(&self, key: &str, value: T) {
        let value = serde_json::to_value(&value).unwrap();
        self.collab_ctx.with_transact_mut(|txn| {
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
            insert_json_value_to_map_ref(key, &value, map_ref.into_inner(), txn);
        }
    }

    pub fn create_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRefWrapper {
        let map = MapPrelim::<lib0::any::Any>::new();
        let map_ref = self.map_ref.insert(txn, key, map);
        MapRefWrapper::new(map_ref, self.collab_ctx.clone())
    }

    pub fn get_map_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<MapRefWrapper> {
        if let Some(Value::YMap(map_ref)) = self.map_ref.get(txn, key) {
            return Some(MapRefWrapper::new(map_ref, self.collab_ctx.clone()));
        }
        None
    }

    pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get_json_with_txn(&self.collab_ctx.transact(), key)
    }

    pub fn get_json_with_txn<T: DeserializeOwned>(
        &self,
        txn: &Transaction,
        key: &str,
    ) -> Option<T> {
        let map_ref = self.get_map_with_txn(txn, key)?;
        let json_value = lib0_any_to_json_value(map_ref.into_inner().to_json(txn)).ok()?;
        serde_json::from_value::<T>(json_value).ok()
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        let txn = self.collab_ctx.transact();
        self.get_str_with_txn(&txn, key)
    }

    pub fn get_str_with_txn(&self, txn: &Transaction, key: &str) -> Option<String> {
        if let Some(Value::Any(Any::String(value))) = self.map_ref.get(txn, key) {
            return Some(value.to_string());
        }
        None
    }

    pub fn get_text_with_txn(&self, txn: &Transaction, key: &str) -> Option<TextRefWrapper> {
        let text_ref = self.map_ref.get(txn, key).map(|value| value.to_ytext())??;
        Some(TextRefWrapper::new(text_ref, self.collab_ctx.clone()))
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

    pub fn transact(&self) -> Transaction {
        self.collab_ctx.transact()
    }

    pub fn with_transact_mut<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut TransactionMut) -> T,
    {
        self.collab_ctx.with_transact_mut(f)
    }

    pub fn to_json(&self) -> String {
        let txn = self.collab_ctx.transact();
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
