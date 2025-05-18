use crate::fields::{Field, field_from_map_ref, field_from_value};
use collab::preclude::{DeepObservable, EntryChange, Event, MapRef, Subscription};
use tokio::sync::broadcast;
use tracing::warn;

pub type FieldChangeSender = broadcast::Sender<FieldChange>;
pub type FieldChangeReceiver = broadcast::Receiver<FieldChange>;

#[derive(Clone, Debug)]
pub enum FieldChange {
  DidCreateField { field: Field },
  DidUpdateField { field: Field },
  DidDeleteField { field_id: String },
}

pub(crate) fn subscribe_field_change(
  field_map: &mut MapRef,
  change_tx: FieldChangeSender,
) -> Subscription {
  field_map.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          let keys = event.keys(txn);
          for (key, value) in keys.iter() {
            let _change_tx = change_tx.clone();
            match value {
              EntryChange::Inserted(value) => {
                // tracing::trace!("field observer: Inserted: {}:{}", key, value);
                if let Some(field) = field_from_value(value.clone(), txn) {
                  let _ = change_tx.send(FieldChange::DidCreateField { field });
                }
              },
              EntryChange::Updated(_, _value) => {
                // tracing::trace!("field observer: update: {}:{}", key, value);
                if let Some(field) = field_from_map_ref(event.target(), txn) {
                  let _ = change_tx.send(FieldChange::DidUpdateField { field });
                }
              },
              EntryChange::Removed(_value) => {
                let field_id = (**key).to_string();
                if !field_id.is_empty() {
                  let _ = change_tx.send(FieldChange::DidDeleteField { field_id });
                } else {
                  warn!("field observer: delete: {}", key);
                }
              },
            }
          }
        },
        _ => {},
      }
    }
  })
}
