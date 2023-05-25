use crate::core::{view_from_map_ref, View, ViewsRelation};
use collab::preclude::array::ArraySubscription;
use collab::preclude::{
  ArrayRefWrapper, Change, DeepEventsSubscription, DeepObservable, EntryChange, Event,
  MapRefWrapper, Observable, ToJson, YrsValue,
};
use std::rc::Rc;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum ViewChange {
  DidCreateView { view: View },
  DidDeleteView { views: Vec<View> },
  DidUpdate { view: View },
}

pub type ViewChangeSender = broadcast::Sender<ViewChange>;
pub type ViewChangeReceiver = broadcast::Receiver<ViewChange>;

#[derive(Debug, Clone)]
pub enum TrashChange {
  DidCreateTrash { ids: Vec<String> },
  DidDeleteTrash { ids: Vec<String> },
}

pub(crate) fn subscribe_folder_change(
  root: &mut MapRefWrapper,
  _change_tx: ViewChangeSender,
) -> DeepEventsSubscription {
  root.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          for c in event.keys(txn).values() {
            match c {
              EntryChange::Inserted(v) => {
                if let YrsValue::YMap(map_ref) = v {
                  tracing::trace!("folder change: Inserted: {}", map_ref.to_json(txn));
                }
              },
              EntryChange::Updated(_k, v) => {
                if let YrsValue::YMap(map_ref) = v {
                  tracing::trace!("folder change: Updated: {}", map_ref.to_json(txn));
                }
              },
              EntryChange::Removed(v) => if let YrsValue::YMap(_map_ref) = v {},
            }
          }
        },
        Event::XmlFragment(_) => {},
        Event::XmlText(_) => {},
      }
    }
  })
}

pub(crate) fn subscribe_view_change(
  root: &mut MapRefWrapper,
  change_tx: ViewChangeSender,
  views_relation: Rc<ViewsRelation>,
) -> DeepEventsSubscription {
  root.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          for c in event.keys(txn).values() {
            let change_tx = change_tx.clone();
            match c {
              EntryChange::Inserted(v) => {
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(view) = view_from_map_ref(map_ref, txn, &views_relation) {
                    let _ = change_tx.send(ViewChange::DidCreateView { view });
                  }
                }
              },
              EntryChange::Updated(_k, v) => {
                println!("update: {}", event.target().to_json(txn));
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(view) = view_from_map_ref(map_ref, txn, &views_relation) {
                    let _ = change_tx.send(ViewChange::DidUpdate { view });
                  }
                }
              },
              EntryChange::Removed(v) => {
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(view) = view_from_map_ref(map_ref, txn, &views_relation) {
                    let _ = change_tx.send(ViewChange::DidDeleteView { views: vec![view] });
                  }
                }
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

pub type TrashChangeSender = broadcast::Sender<TrashChange>;
pub type TrashChangeReceiver = broadcast::Receiver<TrashChange>;

pub(crate) fn subscribe_trash_change(
  array: &mut ArrayRefWrapper,
  _change_tx: TrashChangeSender,
) -> ArraySubscription {
  array.observe(move |txn, event| {
    for change in event.delta(txn) {
      match change {
        Change::Added(_) => {
          // let records = values
          //     .iter()
          //     .flat_map(|value| match value {
          //         Value::Any(any) => Some(any),
          //         _ => None,
          //     })
          //     .map(|any| TrashRecord::from(any.clone()))
          //     .map(|record| record.id)
          //     .collect::<Vec<String>>();
          // let _ = tx.send(TrashChange::DidCreateTrash { ids: records });
        },
        Change::Removed(_) => {},
        Change::Retain(_) => {},
      }
    }
  })
}
