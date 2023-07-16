use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use collab::preclude::array::ArraySubscription;
use collab::preclude::{
  ArrayRefWrapper, Change, DeepEventsSubscription, DeepObservable, EntryChange, Event,
  MapRefWrapper, Observable, ToJson, YrsValue,
};
use parking_lot::RwLock;
use tokio::sync::broadcast;

use crate::core::{view_from_map_ref, View, ViewRelations};

#[derive(Debug, Clone)]
pub enum ViewChange {
  DidCreateView { view: View },
  DidDeleteView { views: Vec<Arc<View>> },
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
  view_cache: Arc<RwLock<HashMap<String, Arc<View>>>>,
  change_tx: ViewChangeSender,
  view_relations: Rc<ViewRelations>,
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
                  if let Some(view) = view_from_map_ref(map_ref, txn, &view_relations) {
                    view_cache
                      .write()
                      .insert(view.id.clone(), Arc::new(view.clone()));
                    let _ = change_tx.send(ViewChange::DidCreateView { view });
                  }
                }
              },
              EntryChange::Updated(_, _) => {
                if let Some(view) = view_from_map_ref(event.target(), txn, &view_relations) {
                  view_cache
                    .write()
                    .insert(view.id.clone(), Arc::new(view.clone()));
                  let _ = change_tx.send(ViewChange::DidUpdate { view });
                }
              },
              EntryChange::Removed(_) => {
                let views = event
                  .keys(txn)
                  .iter()
                  .flat_map(|(k, _)| view_cache.write().remove(&**k))
                  .collect::<Vec<Arc<View>>>();

                if !views.is_empty() {
                  let _ = change_tx.send(ViewChange::DidDeleteView { views });
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
