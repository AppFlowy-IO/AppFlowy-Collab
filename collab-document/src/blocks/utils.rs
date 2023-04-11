use crate::blocks::{ArrayDelta, Delta, MapDelta};
use crate::error::DocumentError;
use collab::preclude::array::ArrayEvent;
use collab::preclude::map::MapEvent;
use collab::preclude::{Change, EntryChange, Event, ToJson, TransactionMut};
use serde_json::Value;
use std::collections::HashMap;

pub fn json_str_to_hashmap(json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
  let v = serde_json::from_str(json_str);
  v.map_err(|_| DocumentError::ConvertDataError)
}

pub fn hashmap_to_json_str(data: HashMap<String, Value>) -> Result<String, DocumentError> {
  let v = serde_json::to_string(&data);
  v.map_err(|_| DocumentError::ConvertDataError)
}

fn set_array_delta_from_event(txn: &TransactionMut, event: &ArrayEvent, delta: &mut Vec<Delta>) {
  event.delta(txn).iter().for_each(|change| {
    let array_change = match change {
      Change::Added(v) => {
        let add_vals = v.iter().map(|v| v.to_json(txn).to_string()).collect();
        ArrayDelta::Added(add_vals)
      },
      Change::Removed(v) => ArrayDelta::Removed(v.to_owned()),
      Change::Retain(v) => ArrayDelta::Retain(v.to_owned()),
    };
    delta.push(Delta::Array(array_change));
  })
}

fn set_map_delta_from_event(txn: &TransactionMut, event: &MapEvent, delta: &mut Vec<Delta>) {
  event.keys(txn).iter().for_each(|(k, v)| {
    let map_change = match v {
      EntryChange::Inserted(value) => MapDelta::Inserted(
        k.to_string(),
        serde_json::to_value(value.to_json(txn)).unwrap_or_default(),
      ),
      EntryChange::Updated(old, new) => MapDelta::Updated(
        k.to_string(),
        serde_json::to_value(old.to_json(txn)).unwrap_or_default(),
        serde_json::to_value(new.to_json(txn)).unwrap_or_default(),
      ),
      EntryChange::Removed(_) => MapDelta::Removed(k.to_string()),
    };
    delta.push(Delta::Map(map_change));
  });
}

pub fn get_delta_from_event(txn: &TransactionMut, event: &Event) -> Vec<Delta> {
  let mut delta = vec![];
  match event {
    Event::Array(val) => {
      set_array_delta_from_event(txn, val, &mut delta);
    },
    Event::Map(val) => {
      set_map_delta_from_event(txn, val, &mut delta);
    },
    _ => {},
  };
  delta
}
