use crate::rows::Row;
use collab::preclude::{
  DeepEventsSubscription, DeepObservable, EntryChange, Event, MapRefWrapper, Value,
};
use std::rc::Rc;
use tokio::sync::broadcast;

pub type RowChangeSender = broadcast::Sender<RowChange>;
pub type RowChangeReceiver = broadcast::Receiver<RowChange>;

#[derive(Debug, Clone)]
pub enum RowChange {
  DidCreateRow { row: Row },
  DidDeleteRow { row: Row },
  DidUpdateRowData { key: Rc<str>, value: Value },
  DidUpdateRowComment { row: Row },
}

pub(crate) fn subscribe_row_data_change(
  row_data_map: &mut MapRefWrapper,
  change_tx: RowChangeSender,
) -> DeepEventsSubscription {
  row_data_map.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          let keys = event.keys(txn);
          for (key, value) in keys.iter() {
            let change_tx = change_tx.clone();
            match value {
              EntryChange::Inserted(value) => {
                tracing::trace!("row observer: Inserted: {}:{}", key, value);
              },
              EntryChange::Updated(_, value) => {
                tracing::trace!("row observer: update: {}:{}", key, value);
              },
              EntryChange::Removed(value) => {
                tracing::trace!("row observer: delete: {}", key);
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
