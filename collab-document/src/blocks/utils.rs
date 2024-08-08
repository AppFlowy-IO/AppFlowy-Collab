use crate::blocks::text_entities::TextDelta;
use crate::blocks::{BlockEvent, BlockEventPayload, DeltaType};
use crate::error::DocumentError;
use collab::preclude::text::YChange;
use collab::preclude::{
  Array, Delta, EntryChange, Event, Map, PathSegment, ReadTxn, Text, TextRef, TransactionMut,
  YrsDelta, YrsValue,
};
use serde_json::Value;
use std::collections::HashMap;

/// block data json string to hashmap
pub fn json_str_to_hashmap(json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
  serde_json::from_str(json_str).map_err(|_| DocumentError::ConvertDataError)
}

/// block data hashmap to json string
pub fn hashmap_to_json_str(data: HashMap<String, Value>) -> Result<String, DocumentError> {
  serde_json::to_string(&data).map_err(|_| DocumentError::ConvertDataError)
}

/// parse block change event to BlockEvent
pub fn parse_event(_object_id: &str, txn: &TransactionMut, event: &Event) -> BlockEvent {
  let path = event
    .path()
    .iter()
    .map(|v| match v {
      PathSegment::Key(v) => v.to_string(),
      PathSegment::Index(v) => v.to_string(),
    })
    .collect::<Vec<String>>();
  let delta = match event {
    Event::Text(val) => {
      // Extract the ID from the last element of the "path" vector
      let id = path.last().map(|v| v.to_string()).unwrap_or_default();

      // Calculate the delta for the "val" using the transaction "txn"
      let delta = val
        .delta(txn)
        .iter()
        .map(|v| TextDelta::from(v.clone().map(|d|d.to_string(txn)))) // Map each delta value to a TextDelta
        .collect::<Vec<TextDelta>>(); // Collect the TextDelta values into a vector

      #[cfg(feature = "verbose_log")]
      tracing::trace!("{}: receive text event: {:?}", _object_id, delta);

      // Serialize the delta vector to a JSON string or use an empty string if there's an error
      let value = serde_json::to_string(&delta).unwrap_or_default();

      // Create a vector containing a BlockEventPayload with the computed values
      vec![BlockEventPayload {
        value,
        id,
        path,
        command: DeltaType::Updated,
      }]
    },
    Event::Array(_val) => {
      let id = path.last().map(|v| v.to_string()).unwrap_or_default();
      let value = vec![BlockEventPayload {
        value: parse_yrs_value(txn, &event.target()),
        id,
        path,
        command: DeltaType::Updated,
      }];

      #[cfg(feature = "verbose_log")]
      tracing::trace!("{}: receive array event: {:?}", _object_id, value);

      value
    },
    Event::Map(val) => {
      let value = val
        .keys(txn)
        .iter()
        .map(|(key, change)| match change {
          EntryChange::Inserted(value) => BlockEventPayload {
            value: parse_yrs_value(txn, value),
            id: key.to_string(),
            path: path.clone(),
            command: DeltaType::Inserted,
          },
          EntryChange::Updated(_, _value) => {
            let id = path.last().map(|v| v.to_string()).unwrap_or_default();

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
        })
        .collect::<Vec<BlockEventPayload>>();

      #[cfg(feature = "verbose_log")]
      tracing::trace!("{}: receive map event: {:?}", _object_id, value);

      value
    },
    _ => vec![],
  };
  BlockEvent::new(delta)
}

/// parse YrsValue to json string
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
    YrsValue::YText(val) => {
      let delta: Vec<TextDelta> = get_delta_with_text_ref(val, txn)
        .iter()
        .map(|v| TextDelta::from(v.clone().map(|d| d.to_string(txn))))
        .collect();
      serde_json::to_string(&delta).unwrap_or_default()
    },
    _ => "".to_string(),
  }
}

pub fn get_delta_with_text_ref<T: ReadTxn>(text_ref: &TextRef, txn: &T) -> Vec<Delta> {
  text_ref
    .diff(txn, YChange::identity)
    .into_iter()
    .map(|change| YrsDelta::Inserted(change.insert, change.attributes))
    .collect()
}

pub fn deserialize_text_delta(delta: &str) -> serde_json::Result<Vec<TextDelta>> {
  serde_json::from_str::<Vec<TextDelta>>(delta)
}
