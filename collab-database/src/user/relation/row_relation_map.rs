use std::ops::Deref;

use collab::preclude::{
  DeepEventsSubscription, DeepObservable, EntryChange, Event, Map, MapRefWrapper, ToJson,
  TransactionMut, YrsValue,
};
use tokio::sync::broadcast;

use crate::user::relation::{RowRelation, RowRelationBuilder};
use crate::user::row_relation_from_map_ref;

#[derive(Debug, Clone)]
pub enum RowRelationChange {
  NewRelation(RowRelation),
  DeleteRelation(RowRelation),
}

pub type RowRelationUpdateSender = broadcast::Sender<RowRelationChange>;
pub type RowRelationUpdateReceiver = broadcast::Receiver<RowRelationChange>;

pub struct RowRelationMap {
  container: MapRefWrapper,
  tx: RowRelationUpdateSender,
  #[allow(dead_code)]
  subscription: DeepEventsSubscription,
}

impl RowRelationMap {
  pub fn from_map_ref(mut container: MapRefWrapper) -> Self {
    let (tx, _) = broadcast::channel(1000);
    let subscription = subscription_changes(tx.clone(), &mut container);
    Self {
      container,
      tx,
      subscription,
    }
  }

  pub fn subscript_update(&self) -> RowRelationUpdateReceiver {
    self.tx.subscribe()
  }

  pub fn insert_relation(&self, relation: RowRelation) {
    self
      .container
      .with_transact_mut(|txn| self.insert_relation_with_txn(txn, relation))
  }

  pub fn insert_relation_with_txn(&self, txn: &mut TransactionMut, relation: RowRelation) {
    let map_ref = self.container.create_map_with_txn(txn, &relation.id());
    RowRelationBuilder::new(
      &relation.linking_database_id,
      &relation.linked_by_database_id,
      txn,
      map_ref,
    )
    .update(|update| {
      update.set_row_connections(relation.row_connections);
    });
  }

  pub fn remove_relation(&self, relation_id: &str) {
    self.container.with_transact_mut(|txn| {
      self.remove_relation_with_txn(txn, relation_id);
    })
  }

  pub fn remove_relation_with_txn(&self, txn: &mut TransactionMut, relation_id: &str) {
    self.container.remove(txn, relation_id);
  }
}

fn subscription_changes(
  tx: RowRelationUpdateSender,
  container: &mut MapRefWrapper,
) -> DeepEventsSubscription {
  container.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          for c in event.keys(txn).values() {
            match c {
              EntryChange::Inserted(v) => {
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(row_relation) = row_relation_from_map_ref(txn, map_ref) {
                    tracing::trace!("insert: {:?}", row_relation);
                    let _ = tx.send(RowRelationChange::NewRelation(row_relation));
                  }
                }
              },
              EntryChange::Updated(_k, _v) => {
                println!("update: {}", event.target().to_json(txn));
              },
              EntryChange::Removed(v) => {
                println!("remove: {}", event.target().to_json(txn));
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(row_relation) = row_relation_from_map_ref(txn, map_ref) {
                    tracing::trace!("delete: {:?}", row_relation);
                    let _ = tx.send(RowRelationChange::DeleteRelation(row_relation));
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

impl Deref for RowRelationMap {
  type Target = MapRefWrapper;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
