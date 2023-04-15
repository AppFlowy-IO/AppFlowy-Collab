use crate::blocks::{BlockEvent, BlockEventPayload, DeltaType};
use crate::error::DocumentError;
use collab::preclude::{EntryChange, Event, PathSegment, ToJson, TransactionMut};
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
        value: event.target().to_string(txn),
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
            value: value.to_json(txn).to_string(),
            id: key.to_string(),
            path: path.clone(),
            command: DeltaType::Inserted,
          },
          EntryChange::Updated(_, _value) => {
            // Here use unwrap is safe, because we have checked the type of event.
            let id = path.last().unwrap().to_string();
            BlockEventPayload {
              value: event.target().to_string(txn),
              id,
              path: path.clone(),
              command: DeltaType::Updated,
            }
          },
          EntryChange::Removed(value) => BlockEventPayload {
            value: value.to_json(txn).to_string(),
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
