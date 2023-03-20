use anyhow::Result;
use lib0::any::Any;
use serde_json::Value as JsonValue;
use yrs::{Map, MapPrelim, MapRef, TransactionMut};

pub fn insert_json_value_to_map_ref(
    key: &str,
    value: &JsonValue,
    map_ref: MapRef,
    txn: &mut TransactionMut,
) {
    if value.is_object() {
        value
            .as_object()
            .unwrap()
            .into_iter()
            .for_each(|(key, value)| {
                let new_map_ref = if value.is_object() {
                    map_ref.insert(txn, key.as_str(), MapPrelim::<lib0::any::Any>::new());
                    map_ref
                        .get(txn, key)
                        .map(|value| value.to_ymap().unwrap())
                        .unwrap()
                } else {
                    map_ref.clone()
                };

                insert_json_value_to_map_ref(key, value, new_map_ref, txn);
            });
    } else {
        match json_value_to_lib0_any(value.clone()) {
            Ok(value) => {
                map_ref.insert(txn, key, value);
            }
            Err(e) => tracing::error!("{:?}", e),
        }
    }
}

pub fn json_value_to_lib0_any(json_value: JsonValue) -> Result<Any> {
    let value = serde_json::from_value(json_value)?;
    Ok(value)
}

pub fn lib0_any_to_json_value(any: Any) -> Result<JsonValue> {
    let json_value = serde_json::to_value(&any)?;
    Ok(json_value)
}
