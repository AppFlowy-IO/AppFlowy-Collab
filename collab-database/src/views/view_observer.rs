use collab::preclude::array::ArrayEvent;
use collab::preclude::{Change, ToJson};
use collab::preclude::{
  DeepEventsSubscription, DeepObservable, EntryChange, Event, MapRefWrapper, PathSegment,
};
use std::ops::Deref;
use std::rc::Rc;
use tokio::sync::broadcast;
use tracing::trace;

use crate::views::{
  row_order_from_value, FieldOrder, FilterMap, GroupMap, LayoutSetting, RowOrder, SortMap,
  ROW_ORDERS,
};

#[derive(Debug, Clone)]
pub enum DatabaseViewChange {
  LayoutSettingChanged {
    view_id: String,
    setting: LayoutSetting,
  },
  DidInsertRowOrders {
    row_orders: Vec<RowOrder>,
  },
  DidDeleteRowAtIndex {
    index: u32,
  },
  // filter
  DidCreateFilter {
    view_id: String,
    filter: FilterMap,
  },
  DidDeleteFilter {
    view_id: String,
    filter: FilterMap,
  },
  DidUpdateFilter {
    view_id: String,
    filter: FilterMap,
  },
  // group
  DidCreateGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  DidDeleteGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  DidUpdateGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  // Sort
  DidCreateSort {
    view_id: String,
    sort: SortMap,
  },
  DidDeleteSort {
    view_id: String,
    sort: SortMap,
  },
  DidUpdateSort {
    view_id: String,
    sort: SortMap,
  },
  // field order
  DidCreateFieldOrder {
    view_id: String,
    field_order: FieldOrder,
  },
  DidDeleteFieldOrder {
    view_id: String,
    field_order: FieldOrder,
  },
}

pub type ViewChangeSender = broadcast::Sender<DatabaseViewChange>;
pub type ViewChangeReceiver = broadcast::Receiver<DatabaseViewChange>;

pub(crate) fn subscribe_view_change(
  view_map: &mut MapRefWrapper,
  change_tx: ViewChangeSender,
) -> DeepEventsSubscription {
  view_map.observe_deep(move |txn, events| {
    for event in events.iter() {
      trace!(
        "view observe event: {:?}, {:?}",
        event.path(),
        event.target().to_json(txn)
      );
      match event {
        Event::Text(_) => {},
        Event::Array(array_event) => {
          let key = ArrayChangeKey::from(array_event);
          array_event
            .delta(txn)
            .iter()
            .for_each(|change| match change {
              Change::Added(values) => match &key {
                ArrayChangeKey::RowOrder => {
                  let row_orders = values
                    .iter()
                    .flat_map(|value| row_order_from_value(value, txn))
                    .collect::<Vec<_>>();
                  let _ = change_tx.send(DatabaseViewChange::DidInsertRowOrders { row_orders });
                },
                ArrayChangeKey::Unknown(s) => {
                  trace!("database view observe unknown insert: {}", s);
                },
              },
              Change::Removed(index) => {
                //
                trace!(
                  "database view observe array delete: {:?}:{:?}",
                  array_event.path(),
                  array_event.target().to_json(txn)
                );
                match &key {
                  ArrayChangeKey::Unknown(s) => {
                    trace!("database view observe unknown remove: {}", s);
                  },
                  ArrayChangeKey::RowOrder => {
                    trace!(
                      "database view observe array delete: {:?}:{}",
                      array_event.path(),
                      index,
                    );
                    let _ =
                      change_tx.send(DatabaseViewChange::DidDeleteRowAtIndex { index: *index });
                  },
                }
              },
              Change::Retain(value) => match &key {
                ArrayChangeKey::Unknown(s) => {},
                ArrayChangeKey::RowOrder => {
                  trace!(
                    "database view observe array retain: {:?}:{:?}",
                    array_event.path(),
                    event.target().to_json(txn)
                  );
                },
              },
            });
        },
        Event::Map(event) => {
          let keys = event.keys(txn);
          for (key, value) in keys.iter() {
            let change_tx = change_tx.clone();
            match value {
              EntryChange::Inserted(value) => {
                trace!(
                  "database view map inserted: {}:{:?}",
                  key,
                  value.to_json(txn)
                );
              },
              EntryChange::Updated(_, value) => {
                trace!("database view map update: {}:{}", key, value);
              },
              EntryChange::Removed(value) => {
                trace!("database view map delete: {}", key);
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

enum ArrayChangeKey {
  Unknown(String),
  RowOrder,
}

impl From<&ArrayEvent> for ArrayChangeKey {
  fn from(event: &ArrayEvent) -> Self {
    match event.path().pop_back() {
      Some(segment) => match segment {
        PathSegment::Key(s) => match s.deref() {
          ROW_ORDERS => Self::RowOrder,
          _ => Self::Unknown(s.deref().to_string()),
        },
        PathSegment::Index(_) => Self::Unknown("index".to_string()),
      },
      None => Self::Unknown("empty path".to_string()),
    }
  }
}
