use dashmap::DashMap;
use std::sync::Arc;

use collab::preclude::{DeepObservable, EntryChange, Event, MapRef, Subscription, YrsValue};
use tokio::sync::broadcast;

use crate::section::SectionMap;
use crate::{ParentChildRelations, View, view_from_map_ref};

#[derive(Debug, Clone)]
pub enum ViewChange {
  DidCreateView { view: View },
  DidDeleteView { views: Vec<Arc<View>> },
  DidUpdate { view: View },
}

pub type ViewChangeSender = broadcast::Sender<ViewChange>;
pub type ViewChangeReceiver = broadcast::Receiver<ViewChange>;

pub(crate) fn subscribe_view_change(
  root: &MapRef,
  deletion_cache: Arc<DashMap<String, Arc<View>>>,
  change_tx: ViewChangeSender,
  view_relations: Arc<ParentChildRelations>,
  section_map: Arc<SectionMap>,
  uid: i64,
) -> Subscription {
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
                  if let Some(view) =
                    view_from_map_ref(map_ref, txn, &view_relations, &section_map, uid)
                  {
                    deletion_cache.insert(view.id.clone(), Arc::new(view.clone()));

                    // Send indexing view
                    let _ = change_tx.send(ViewChange::DidCreateView { view });
                  }
                }
              },
              EntryChange::Updated(_, _) => {
                if let Some(view) =
                  view_from_map_ref(event.target(), txn, &view_relations, &section_map, uid)
                {
                  // Update deletion cache with the updated view
                  deletion_cache.insert(view.id.clone(), Arc::new(view.clone()));
                  let _ = change_tx.send(ViewChange::DidUpdate { view });
                }
              },
              EntryChange::Removed(_) => {
                let deleted_views: Vec<Arc<View>> = event
                  .keys(txn)
                  .iter()
                  .filter_map(|(k, _)| deletion_cache.remove(&**k).map(|v| v.1))
                  .collect();

                let delete_ids: Vec<String> = event
                  .keys(txn)
                  .iter()
                  .map(|(k, _)| (**k).to_owned())
                  .collect();

                if !delete_ids.is_empty() {
                  let _ = change_tx.send(ViewChange::DidDeleteView {
                    views: deleted_views,
                  });
                }
              },
            }
          }
        },
        Event::XmlFragment(_) => {},
        Event::XmlText(_) => {},
        #[allow(unreachable_patterns)]
        _ => {},
      }
    }
  })
}
