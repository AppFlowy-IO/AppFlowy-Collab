use crate::core::origin::CollabOrigin;
use crate::database::rows::comment::RowComment;
use crate::database::rows::{
  Cell, ROW_CELLS, ROW_HEIGHT, ROW_VISIBILITY, RowMetaKey, meta_id_from_row_id,
};
use crate::entity::uuid_validation::RowId;

use crate::preclude::{DeepObservable, EntryChange, Event, MapRef, TransactionMut};
use crate::preclude::{PathSegment, ToJson};
use std::ops::Deref;

use crate::preclude::map::MapEvent;
use crate::util::AnyExt;
use tokio::sync::broadcast;
use tracing::trace;

pub type RowChangeSender = broadcast::Sender<RowChange>;
pub type RowChangeReceiver = broadcast::Receiver<RowChange>;

/// Key constant for the comments map in the row document
pub const ROW_COMMENTS: &str = "comment";

#[derive(Debug, Clone)]
pub enum RowChange {
  DidUpdateVisibility {
    row_id: RowId,
    value: bool,
    is_local_change: bool,
  },
  DidUpdateHeight {
    row_id: RowId,
    value: i32,
    is_local_change: bool,
  },
  DidUpdateCell {
    row_id: RowId,
    field_id: String,
    value: Cell,
    is_local_change: bool,
  },
  DidUpdateRowMeta {
    row_id: RowId,
    is_local_change: bool,
  },
  /// A comment was added to the row
  DidAddComment {
    row_id: RowId,
    comment: RowComment,
    is_local_change: bool,
  },
  /// A comment was updated
  DidUpdateComment {
    row_id: RowId,
    comment: RowComment,
    is_local_change: bool,
  },
  /// A comment was deleted
  DidDeleteComment {
    row_id: RowId,
    comment_id: String,
    is_local_change: bool,
  },
}

pub(crate) fn subscribe_row_data_change(
  origin: CollabOrigin,
  row_id: RowId,
  row_data_map: &MapRef,
  change_tx: RowChangeSender,
) {
  row_data_map.observe_deep_with("change", move |txn, events| {
    let txn_origin = CollabOrigin::from(txn);
    let is_local_change = txn_origin == origin;
    for event in events.iter() {
      match event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(map_event) => {
          handle_map_event(&row_id, &change_tx, is_local_change, txn, event, map_event);
        },
        Event::XmlFragment(_) => {},
        Event::XmlText(_) => {},
        #[allow(unreachable_patterns)]
        _ => {},
      }
    }
  });
}

pub(crate) fn subscribe_row_meta_change(
  origin: CollabOrigin,
  row_id: RowId,
  row_meta_map: &MapRef,
  change_tx: RowChangeSender,
) {
  let is_document_empty_key = meta_id_from_row_id(&row_id, RowMetaKey::IsDocumentEmpty);
  row_meta_map.observe_deep_with("meta-change", move |txn, events| {
    let txn_origin = CollabOrigin::from(txn);
    let is_local_change = txn_origin == origin;
    for event in events.iter() {
      if let Event::Map(map_event) = event {
        for (key, entry_change) in map_event.keys(txn).iter() {
          if key.deref() != is_document_empty_key {
            continue;
          }
          if matches!(
            entry_change,
            EntryChange::Inserted(_) | EntryChange::Updated(_, _) | EntryChange::Removed(_)
          ) {
            let _ = change_tx.send(RowChange::DidUpdateRowMeta {
              row_id,
              is_local_change,
            });
            break;
          }
        }
      }
    }
  });
}

/// Subscribe to comment changes in a row.
/// Emits RowChange events for comment additions, updates, and deletions.
pub fn subscribe_row_comment_change(
  origin: CollabOrigin,
  row_id: RowId,
  comments_map: &MapRef,
  change_tx: RowChangeSender,
) {
  comments_map.observe_deep_with("comment-change", move |txn, events| {
    let txn_origin = CollabOrigin::from(txn);
    let is_local_change = txn_origin == origin;
    for event in events.iter() {
      if let Event::Map(map_event) = event {
        for (key, entry_change) in map_event.keys(txn).iter() {
          let comment_id = key.deref().to_string();
          match entry_change {
            EntryChange::Inserted(value) => {
              // A new comment was added
              if let Ok(comment_map) = value.clone().cast::<MapRef>() {
                if let Some(comment) = RowComment::from_map_ref(&comment_map, txn) {
                  let _ = change_tx.send(RowChange::DidAddComment {
                    row_id,
                    comment,
                    is_local_change,
                  });
                }
              }
            },
            EntryChange::Updated(_, value) => {
              // A comment was updated
              if let Ok(comment_map) = value.clone().cast::<MapRef>() {
                if let Some(comment) = RowComment::from_map_ref(&comment_map, txn) {
                  let _ = change_tx.send(RowChange::DidUpdateComment {
                    row_id,
                    comment,
                    is_local_change,
                  });
                }
              }
            },
            EntryChange::Removed(_) => {
              // A comment was deleted
              let _ = change_tx.send(RowChange::DidDeleteComment {
                row_id,
                comment_id,
                is_local_change,
              });
            },
          }
        }
      }
    }
  });
}

fn handle_map_event(
  row_id: &RowId,
  change_tx: &RowChangeSender,
  is_local_change: bool,
  txn: &TransactionMut,
  event: &Event,
  map_event: &MapEvent,
) {
  let path = RowChangePath::from(event);
  for (key, enctry_change) in map_event.keys(txn).iter() {
    match &path {
      RowChangePath::Unknown(_s) => {
        // When the event path is identified as [RowChangePath::Unknown], it indicates that the path itself remains unchanged.
        // In this scenario, the modification is confined to the key/value pairs within the map at the existing path.
        // Essentially, even though the overall path stays the same, the contents (specific key/value pairs) at this path are the ones being updated.
        if let EntryChange::Updated(_, value) = enctry_change {
          let change_value = RowChangeValue::from(key.deref());
          match change_value {
            RowChangeValue::Unknown(_s) => {
              trace!("row observe value update: {}:{:?}", key, value.to_json(txn))
            },
            RowChangeValue::Height => {
              if let Ok(value) = value.clone().cast::<i64>() {
                let _ = change_tx.send(RowChange::DidUpdateHeight {
                  row_id: *row_id,
                  value: value as i32,
                  is_local_change,
                });
              }
            },
            RowChangeValue::Visibility => {
              if let Ok(value) = value.clone().cast::<bool>() {
                let _ = change_tx.send(RowChange::DidUpdateVisibility {
                  row_id: *row_id,
                  value,
                  is_local_change,
                });
              }
            },
          }
        }
      },
      RowChangePath::Cells => {
        match enctry_change {
          EntryChange::Inserted(value) => {
            trace!("row observe insert: {}", key);
            // When a cell's value is newly inserted, the corresponding event exhibits specific characteristics:
            // - The event path is set to "/cells", indicating the operation is within the cells structure.
            // - The 'key' in the event corresponds to the unique identifier of the newly inserted cell.
            // - The 'value' represents the actual content or data inserted into this cell.
            if let Some(cell) = value.to_json(txn).into_map() {
              // when insert a cell into the row, the key is the field_id
              let field_id = key.to_string();
              let _ = change_tx.send(RowChange::DidUpdateCell {
                row_id: *row_id,
                field_id,
                value: cell,
                is_local_change,
              });
            }
          },
          EntryChange::Updated(_, _) => {
            // Processing an update to a cell's value:
            // The event path for an updated cell value is structured as "/cells/{key}", where {key} is the unique identifier of the cell.
            // The 'target' of the event represents the new, updated value of the cell.
            // To accurately identify which cell has been updated, we need to extract its key from the event path.
            // This extraction is achieved by removing the last segment of the path, which is "/{key}".
            // After this removal, the remaining part of the path directly corresponds to the key of the cell.
            // In the current implementation, this key is used as the identifier (ID) of the field within the cells map.
            if let Some(PathSegment::Key(key)) = event.path().pop_back() {
              if let Some(cell) = event.target().to_json(txn).into_map() {
                let field_id = key.deref().to_string();
                let _ = change_tx.send(RowChange::DidUpdateCell {
                  row_id: *row_id,
                  field_id,
                  value: cell,
                  is_local_change,
                });
              }
            }
          },
          EntryChange::Removed(_value) => {
            trace!("row observe delete: {}", key);
            if let Some(PathSegment::Key(key)) = event.path().pop_back() {
              let field_id = key.deref().to_string();
              let _ = change_tx.send(RowChange::DidUpdateCell {
                row_id: *row_id,
                field_id,
                value: Cell::default(),
                is_local_change,
              });
            }
          },
        }
      },
      RowChangePath::Comments => {
        // Comment changes are handled by subscribe_row_comment_change
        // This path is here for completeness but actual handling is done
        // via the dedicated comment observer
        trace!("row observe comment change: {}", key);
      },
    }
  }
}

enum RowChangePath {
  Unknown(String),
  Cells,
  Comments,
}

impl From<&Event> for RowChangePath {
  fn from(event: &Event) -> Self {
    match event.path().pop_front() {
      Some(segment) => match segment {
        PathSegment::Key(s) => RowChangePath::from(s.deref()),
        PathSegment::Index(_) => Self::Unknown("index".to_string()),
      },
      None => Self::Unknown("".to_string()),
    }
  }
}

impl From<&str> for RowChangePath {
  fn from(s: &str) -> Self {
    match s {
      ROW_CELLS => Self::Cells,
      ROW_COMMENTS => Self::Comments,
      s => Self::Unknown(s.to_string()),
    }
  }
}
enum RowChangeValue {
  Unknown(String),
  Height,
  Visibility,
}

impl From<&str> for RowChangeValue {
  fn from(s: &str) -> Self {
    match s {
      ROW_HEIGHT => Self::Height,
      ROW_VISIBILITY => Self::Visibility,
      s => Self::Unknown(s.to_string()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::origin::{CollabClient, CollabOrigin};
  use crate::database::rows::{CellBuilder, CellsUpdate};
  use crate::preclude::{Any, Doc, Map, MapExt, Transact};
  use std::collections::HashMap;
  use std::time::Duration;
  use tokio::sync::broadcast;
  use tokio::time::timeout;
  use uuid::Uuid;

  const CHANGE_TIMEOUT: Duration = Duration::from_secs(2);

  async fn recv_with_timeout(rx: &mut RowChangeReceiver) -> RowChange {
    timeout(CHANGE_TIMEOUT, rx.recv())
      .await
      .expect("timed out waiting for row change")
      .expect("row change channel closed unexpectedly")
  }

  fn drain(rx: &mut RowChangeReceiver) {
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
  async fn row_observer_emits_cell_height_and_visibility_changes() {
    let doc = Doc::new();
    let row_data_map: MapRef = doc.get_or_insert_map("row_data");
    let row_id = Uuid::new_v4();
    let (origin, _) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    subscribe_row_data_change(origin.clone(), row_id, &row_data_map, change_tx);

    let field_id = Uuid::new_v4().to_string();
    let initial_cell: CellBuilder = HashMap::from([
      ("field_type".into(), Any::BigInt(0)),
      ("content".into(), Any::from("v1")),
    ]);

    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let _cells_map: MapRef = row_data_map.get_or_init(&mut txn, ROW_CELLS);
    }

    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let cells_map: MapRef = row_data_map
        .get_with_txn(&txn, ROW_CELLS)
        .expect("missing cells map");
      CellsUpdate::new(&mut txn, &cells_map).insert_cell(&field_id, initial_cell);
    }

    let inserted = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateCell { .. }) {
        break change;
      }
    };
    match inserted {
      RowChange::DidUpdateCell {
        row_id: changed_row_id,
        field_id: changed_field_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
        assert!(is_local_change);
        assert_eq!(
          value
            .get("content")
            .and_then(|v| v.clone().cast::<String>().ok()),
          Some("v1".to_string())
        );
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let cells_map: MapRef = row_data_map
        .get_with_txn(&txn, ROW_CELLS)
        .expect("missing cells map");
      CellsUpdate::new(&mut txn, &cells_map).insert(
        &field_id,
        HashMap::from([("content".into(), Any::from("v2"))]),
      );
    }

    let updated = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateCell { .. }) {
        break change;
      }
    };
    match updated {
      RowChange::DidUpdateCell {
        row_id: changed_row_id,
        field_id: changed_field_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
        assert!(is_local_change);
        assert_eq!(
          value
            .get("content")
            .and_then(|v| v.clone().cast::<String>().ok()),
          Some("v2".to_string())
        );
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let cells_map: MapRef = row_data_map
        .get_with_txn(&txn, ROW_CELLS)
        .expect("missing cells map");
      CellsUpdate::new(&mut txn, &cells_map).clear(&field_id);
    }

    let cleared = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateCell { .. }) {
        break change;
      }
    };
    match cleared {
      RowChange::DidUpdateCell {
        row_id: changed_row_id,
        field_id: changed_field_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
        assert!(is_local_change);
        assert!(value.is_empty());
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      row_data_map.insert(&mut txn, ROW_HEIGHT, Any::BigInt(60));
      row_data_map.insert(&mut txn, ROW_VISIBILITY, true);
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      row_data_map.insert(&mut txn, ROW_HEIGHT, Any::BigInt(120));
    }

    let height_change = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateHeight { .. }) {
        break change;
      }
    };
    match height_change {
      RowChange::DidUpdateHeight {
        row_id: changed_row_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(value, 120);
        assert!(is_local_change);
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      row_data_map.insert(&mut txn, ROW_VISIBILITY, false);
    }

    let visibility_change = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateVisibility { .. }) {
        break change;
      }
    };
    match visibility_change {
      RowChange::DidUpdateVisibility {
        row_id: changed_row_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert!(!value);
        assert!(is_local_change);
      },
      other => panic!("unexpected row change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn row_observer_marks_remote_changes_when_origin_differs() {
    let doc = Doc::new();
    let row_data_map: MapRef = doc.get_or_insert_map("row_data");
    let row_id = Uuid::new_v4();
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    subscribe_row_data_change(origin.clone(), row_id, &row_data_map, change_tx);

    let field_id = Uuid::new_v4().to_string();

    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let _cells_map: MapRef = row_data_map.get_or_init(&mut txn, ROW_CELLS);
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin);
      let cells_map: MapRef = row_data_map
        .get_with_txn(&txn, ROW_CELLS)
        .expect("missing cells map");
      CellsUpdate::new(&mut txn, &cells_map).insert(
        &field_id,
        HashMap::from([("content".into(), Any::from("remote"))]),
      );
    }

    let changed = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateCell { .. }) {
        break change;
      }
    };
    match changed {
      RowChange::DidUpdateCell {
        row_id: changed_row_id,
        field_id: changed_field_id,
        value,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
        assert!(!is_local_change);
        assert_eq!(
          value
            .get("content")
            .and_then(|v| v.clone().cast::<String>().ok()),
          Some("remote".to_string())
        );
      },
      other => panic!("unexpected row change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn row_observer_emits_row_meta_changes() {
    let doc = Doc::new();
    let row_meta_map: MapRef = doc.get_or_insert_map("row_meta");
    let row_id = Uuid::new_v4();
    let (origin, _) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    subscribe_row_meta_change(origin.clone(), row_id, &row_meta_map, change_tx);

    let is_document_empty_key = meta_id_from_row_id(&row_id, RowMetaKey::IsDocumentEmpty);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      row_meta_map.insert(&mut txn, is_document_empty_key, false);
    }

    let change = loop {
      let change = recv_with_timeout(&mut change_rx).await;
      if matches!(change, RowChange::DidUpdateRowMeta { .. }) {
        break change;
      }
    };
    match change {
      RowChange::DidUpdateRowMeta {
        row_id: changed_row_id,
        is_local_change,
      } => {
        assert_eq!(changed_row_id, row_id);
        assert!(is_local_change);
      },
      other => panic!("unexpected row change: {:?}", other),
    }
  }
}
