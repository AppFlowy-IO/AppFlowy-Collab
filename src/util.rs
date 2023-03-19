use crate::collab::Collab;
use lib0::any::Any;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use yrs::{Map, MapPrelim, MapRef, Transact, Transaction, TransactionMut, WriteTxn};

pub(crate) fn insert_value_to_parent(
    id: &str,
    object: &JsonValue,
    parent: Option<MapRef>,
    txn: &mut TransactionMut,
    collab: &Collab,
) {
    let map = match parent {
        None => collab.create_map_with_transaction(id, txn),
        Some(parent) => {
            let map = MapPrelim::<lib0::any::Any>::new();
            if object.is_object() {
                parent.insert(txn, id, map);
                parent
                    .get(txn, id)
                    .map(|value| value.to_ymap().unwrap())
                    .unwrap()
            } else {
                parent
            }
        }
    };
    if object.is_object() {
        object.as_object().unwrap().into_iter().for_each(|(k, v)| {
            insert_value_to_parent(k, v, Some(map.clone()), txn, collab);
        });
    } else {
        map.insert(txn, id, json_value_to_any(object.clone()));
    }
}

fn json_value_to_any(json_value: JsonValue) -> Any {
    match json_value {
        JsonValue::Null => Any::Null,
        JsonValue::Bool(value) => Any::Bool(value),
        JsonValue::Number(value) => {
            if value.is_f64() {
                return Any::Number(value.as_f64().unwrap());
            }
            if value.is_i64() {
                return Any::BigInt(value.as_i64().unwrap());
            }
            if value.is_u64() {
                return Any::BigInt(value.as_u64().unwrap() as i64);
            }
            Any::Null
        }
        JsonValue::String(value) => value.into(),
        JsonValue::Array(values) => values
            .into_iter()
            .map(json_value_to_any)
            .collect::<Vec<Any>>()
            .into(),
        JsonValue::Object(map) => map
            .into_iter()
            .map(|(k, v)| (k, json_value_to_any(v)))
            .collect::<HashMap<String, Any>>()
            .into(),
    }
}
