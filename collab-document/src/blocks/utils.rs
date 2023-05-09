use crate::blocks::{BlockEvent, BlockEventPayload, DeltaType};
use crate::error::DocumentError;
use collab::preclude::{Array, EntryChange, Event, Map, PathSegment, TransactionMut, YrsValue};
use serde_json::Value;
use std::collections::HashMap;

pub fn json_str_to_hashmap(json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
  serde_json::from_str(json_str).map_err(|_| DocumentError::ConvertDataError)
}

pub fn hashmap_to_json_str(data: HashMap<String, Value>) -> Result<String, DocumentError> {
  serde_json::to_string(&data).map_err(|_| DocumentError::ConvertDataError)
}

pub fn parse_event(txn: &TransactionMut, event: &Event) -> BlockEvent {
  let path = event
    .path()
    .iter()
    .map(|v| match v {
      PathSegment::Key(v) => v.to_string(),
      PathSegment::Index(v) => v.to_string(),
    })
    .collect::<Vec<String>>();
  let delta = match event {
    Event::Array(_val) => {
      // Here use unwrap is safe, because we have checked the type of event.
      let id = path.last().unwrap().to_string();

      vec![BlockEventPayload {
        value: parse_yrs_value(txn, &event.target()),
        id,
        path,
        command: DeltaType::Updated,
      }]
    },
    Event::Map(val) => val
      .keys(txn)
      .iter()
      .map(|(key, change)| {
        match change {
          EntryChange::Inserted(value) => BlockEventPayload {
            value: parse_yrs_value(txn, value),
            id: key.to_string(),
            path: path.clone(),
            command: DeltaType::Inserted,
          },
          EntryChange::Updated(_, _value) => {
            // Here use unwrap is safe, because we have checked the type of event.
            let id = path.last().unwrap().to_string();

            BlockEventPayload {
              value: parse_yrs_value(txn, &event.target()),
              id,
              path: path.clone(),
              command: DeltaType::Updated,
            }
          },
          EntryChange::Removed(value) => BlockEventPayload {
            value: parse_yrs_value(txn, value),
            id: key.to_string(),
            path: path.clone(),
            command: DeltaType::Removed,
          },
        }
      })
      .collect::<Vec<BlockEventPayload>>(),
    _ => vec![],
  };
  BlockEvent::new(delta)
}

fn parse_yrs_value(txn: &TransactionMut, value: &YrsValue) -> String {
  match value {
    YrsValue::YArray(val) => {
      let array = val
        .iter(txn)
        .map(|v| v.to_string(txn))
        .collect::<Vec<String>>();
      serde_json::to_string(&array).unwrap_or_default()
    },
    YrsValue::YMap(val) => {
      let obj = val
        .iter(txn)
        .map(|(k, v)| (k.to_string(), v.to_string(txn)))
        .collect::<HashMap<String, String>>();
      serde_json::to_string(&obj).unwrap_or_default()
    },
    _ => "".to_string(),
  }
}
