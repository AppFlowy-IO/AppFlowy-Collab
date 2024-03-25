use crate::fields::Field;
use collab::preclude::{DeepObservable, EntryChange, Event, MapRefWrapper, Subscription};
use tokio::sync::broadcast;

pub type FieldChangeSender = broadcast::Sender<FieldChange>;
pub type FieldChangeReceiver = broadcast::Receiver<FieldChange>;

#[derive(Clone, Debug)]
pub enum FieldChange {
  DidUpdateField(Field),
}

pub(crate) fn subscribe_field_change(
  field_map: &mut MapRefWrapper,
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
                tracing::trace!("field observer: Inserted: {}:{}", key, value);
              },
              EntryChange::Updated(_, value) => {
                tracing::trace!("field observer: update: {}:{}", key, value);
              },
              EntryChange::Removed(_value) => {
                tracing::trace!("field observer: delete: {}", key);
              },
            }
          }
        },
        Event::XmlFragment(_) => {},
        Event::XmlText(_) => {},
      }
    }
  })
}
