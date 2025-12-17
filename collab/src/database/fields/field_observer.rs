use crate::core::origin::CollabOrigin;
use crate::database::fields::{Field, field_from_map_ref, field_from_value};
use crate::preclude::{
  DeepObservable, EntryChange, Event, MapExt, MapRef, PathSegment, Subscription,
};
use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast;
use tracing::warn;

pub type FieldChangeSender = broadcast::Sender<FieldChange>;
pub type FieldChangeReceiver = broadcast::Receiver<FieldChange>;

#[derive(Clone, Debug)]
pub enum FieldChange {
  DidCreateField {
    field: Field,
    is_local_change: bool,
  },
  DidUpdateField {
    field: Field,
    is_local_change: bool,
  },
  DidDeleteField {
    field_id: String,
    is_local_change: bool,
  },
}

pub(crate) fn subscribe_field_change(
  origin: CollabOrigin,
  field_map: &mut MapRef,
  change_tx: FieldChangeSender,
) -> Subscription {
  let field_root = field_map.clone();
  field_map.observe_deep(move |txn, events| {
    let txn_origin = CollabOrigin::from(txn);
    let is_local_change = txn_origin == origin;
    let mut inserted_fields = HashMap::<String, Field>::new();
    let mut deleted_field_ids = HashSet::<String>::new();
    let mut updated_field_ids = HashSet::<String>::new();

    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          let path = event.path();
          if path.is_empty() {
            for (key, value) in event.keys(txn).iter() {
              match value {
                EntryChange::Inserted(value) => {
                  if let Some(field) = field_from_value(value.clone(), txn) {
                    inserted_fields.insert(field.id.clone(), field);
                  }
                },
                EntryChange::Updated(_, value) => {
                  if let Some(field) = field_from_value(value.clone(), txn) {
                    updated_field_ids.insert(field.id);
                  }
                },
                EntryChange::Removed(_value) => {
                  let field_id = (**key).to_string();
                  if !field_id.is_empty() {
                    deleted_field_ids.insert(field_id);
                  } else {
                    warn!("field observer: delete: {}", key);
                  }
                },
              }
            }
          } else if let Some(PathSegment::Key(field_id)) = path.front() {
            updated_field_ids.insert(field_id.to_string());
          }
        },
        _ => {},
      }
    }

    let inserted_field_ids = inserted_fields.keys().cloned().collect::<HashSet<_>>();

    for field in inserted_fields.into_values() {
      let _ = change_tx.send(FieldChange::DidCreateField {
        field,
        is_local_change,
      });
    }

    for field_id in deleted_field_ids.iter() {
      let _ = change_tx.send(FieldChange::DidDeleteField {
        field_id: field_id.clone(),
        is_local_change,
      });
    }

    for field_id in updated_field_ids.into_iter() {
      if deleted_field_ids.contains(&field_id) || inserted_field_ids.contains(&field_id) {
        continue;
      }

      if let Some(map_ref) = field_root.get_with_txn(txn, &field_id) {
        if let Some(field) = field_from_map_ref(&map_ref, txn) {
          let _ = change_tx.send(FieldChange::DidUpdateField {
            field,
            is_local_change,
          });
        }
      }
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::origin::{CollabClient, CollabOrigin};
  use crate::database::fields::{Field, FieldMap};
  use crate::preclude::{Doc, MapRef, Transact};
  use std::time::Duration;
  use tokio::sync::broadcast;
  use tokio::time::timeout;
  use uuid::Uuid;

  const CHANGE_TIMEOUT: Duration = Duration::from_secs(2);

  async fn recv_with_timeout(rx: &mut FieldChangeReceiver) -> FieldChange {
    timeout(CHANGE_TIMEOUT, rx.recv())
      .await
      .expect("timed out waiting for field change")
      .expect("field change channel closed unexpectedly")
  }

  fn drain(rx: &mut FieldChangeReceiver) {
    loop {
      match rx.try_recv() {
        Ok(_) => continue,
        Err(broadcast::error::TryRecvError::Empty) => break,
        Err(broadcast::error::TryRecvError::Closed) => break,
        Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
      }
    }
  }

  fn local_and_remote_origins() -> (CollabOrigin, CollabOrigin) {
    let local = CollabOrigin::Client(CollabClient::new(0xdeadbeef, "local-device"));
    let remote = CollabOrigin::Client(CollabClient::new(0xfeedface, "remote-device"));
    (local, remote)
  }

  #[tokio::test]
  async fn field_create_update_delete_marks_is_local_for_matching_origin() {
    let doc = Doc::new();
    let mut fields_map: MapRef = doc.get_or_insert_map("fields");
    let (origin, _) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(128);
    let _sub = subscribe_field_change(origin.clone(), &mut fields_map, change_tx);

    let field_map = FieldMap::new(origin.clone(), fields_map.clone(), None);
    let field_id = Uuid::new_v4().to_string();
    let field = Field::new(field_id.clone(), "Field".to_string(), 0, false);

    {
      let mut txn = doc.transact_mut_with(origin.clone());
      field_map.insert_field(&mut txn, field);
    }

    let created = recv_with_timeout(&mut change_rx).await;
    match created {
      FieldChange::DidCreateField {
        field,
        is_local_change,
      } => {
        assert_eq!(field.id, field_id);
        assert!(is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      field_map.update_field(&mut txn, &field_id, |update| {
        update.set_name("Renamed");
      });
    }

    let updated = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, FieldChange::DidUpdateField { .. }) {
        break change;
      }
    };
    match updated {
      FieldChange::DidUpdateField {
        field,
        is_local_change,
      } => {
        assert_eq!(field.id, field_id);
        assert_eq!(field.name, "Renamed");
        assert!(is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin);
      field_map.delete_field(&mut txn, &field_id);
    }

    let deleted = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, FieldChange::DidDeleteField { .. }) {
        break change;
      }
    };
    match deleted {
      FieldChange::DidDeleteField {
        field_id: deleted_field_id,
        is_local_change,
      } => {
        assert_eq!(deleted_field_id, field_id);
        assert!(is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn field_changes_mark_remote_when_origin_differs() {
    let doc = Doc::new();
    let mut fields_map: MapRef = doc.get_or_insert_map("fields");
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(128);
    let _sub = subscribe_field_change(origin.clone(), &mut fields_map, change_tx);

    let field_map = FieldMap::new(origin.clone(), fields_map.clone(), None);
    let field_id = Uuid::new_v4().to_string();
    let field = Field::new(field_id.clone(), "Field".to_string(), 0, false);

    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      field_map.insert_field(&mut txn, field);
    }

    let created = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, FieldChange::DidCreateField { .. }) {
        break change;
      }
    };
    match created {
      FieldChange::DidCreateField {
        field,
        is_local_change,
      } => {
        assert_eq!(field.id, field_id);
        assert!(!is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      field_map.update_field(&mut txn, &field_id, |update| {
        update.set_name("Renamed");
      });
    }

    let updated = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, FieldChange::DidUpdateField { .. }) {
        break change;
      }
    };
    match updated {
      FieldChange::DidUpdateField {
        field,
        is_local_change,
      } => {
        assert_eq!(field.id, field_id);
        assert_eq!(field.name, "Renamed");
        assert!(!is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin);
      field_map.delete_field(&mut txn, &field_id);
    }

    let deleted = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, FieldChange::DidDeleteField { .. }) {
        break change;
      }
    };
    match deleted {
      FieldChange::DidDeleteField {
        field_id: deleted_field_id,
        is_local_change,
      } => {
        assert_eq!(deleted_field_id, field_id);
        assert!(!is_local_change);
      },
      other => panic!("unexpected field change: {:?}", other),
    }
  }
}
