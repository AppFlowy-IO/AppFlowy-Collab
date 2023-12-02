use crate::views::{FieldOrder, FilterMap, GroupMap, LayoutSetting, SortMap};
use collab::preclude::{DeepEventsSubscription, DeepObservable, EntryChange, Event, MapRefWrapper};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum DatabaseViewChange {
  LayoutSettingChanged {
    view_id: String,
    setting: LayoutSetting,
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
                tracing::trace!("database view observer: Inserted: {}:{}", key, value);
              },
              EntryChange::Updated(_, value) => {
                tracing::trace!("database view observer: update: {}:{}", key, value);
              },
              EntryChange::Removed(value) => {
                tracing::trace!("datbaase view observer: delete: {}", key);
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
