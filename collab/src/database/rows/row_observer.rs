use crate::database::rows::{Cell, ROW_CELLS, ROW_HEIGHT, ROW_VISIBILITY, Row};
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

#[derive(Debug, Clone)]
pub enum RowChange {
  DidUpdateVisibility {
    row_id: RowId,
    value: bool,
  },
  DidUpdateHeight {
    row_id: RowId,
    value: i32,
  },
  DidUpdateCell {
    row_id: RowId,
    field_id: String,
    value: Cell,
  },
  DidUpdateRowComment {
    row: Row,
  },
}

pub(crate) fn subscribe_row_data_change(
  row_id: RowId,
  row_data_map: &MapRef,
  change_tx: RowChangeSender,
) {
  row_data_map.observe_deep_with("change", move |txn, events| {
    for event in events.iter() {
      match event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(map_event) => {
          handle_map_event(&row_id, &change_tx, txn, event, map_event);
        },
        Event::XmlFragment(_) => {},
        Event::XmlText(_) => {},
        #[allow(unreachable_patterns)]
        _ => {},
      }
    }
  });
}

fn handle_map_event(
  row_id: &RowId,
  change_tx: &RowChangeSender,
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
                });
              }
            },
            RowChangeValue::Visibility => {
              if let Ok(value) = value.clone().cast::<bool>() {
                let _ = change_tx.send(RowChange::DidUpdateVisibility {
                  row_id: *row_id,
                  value,
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
              });
            }
          },
        }
      },
    }
  }
}

enum RowChangePath {
  Unknown(String),
  Cells,
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

  #[tokio::test]
  async fn row_observer_emits_cell_height_and_visibility_changes() {
    let doc = Doc::new();
    let row_data_map: MapRef = doc.get_or_insert_map("row_data");
    let row_id = Uuid::new_v4();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    subscribe_row_data_change(row_id, &row_data_map, change_tx);

    let field_id = Uuid::new_v4().to_string();
    let initial_cell: CellBuilder = HashMap::from([
      ("field_type".into(), Any::BigInt(0)),
      ("content".into(), Any::from("v1")),
    ]);

    {
      let mut txn = doc.transact_mut();
      let _cells_map: MapRef = row_data_map.get_or_init(&mut txn, ROW_CELLS);
    }

    {
      let mut txn = doc.transact_mut();
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
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
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
      let mut txn = doc.transact_mut();
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
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
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
      let mut txn = doc.transact_mut();
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
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(changed_field_id, field_id);
        assert!(value.is_empty());
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut();
      row_data_map.insert(&mut txn, ROW_HEIGHT, Any::BigInt(60));
      row_data_map.insert(&mut txn, ROW_VISIBILITY, true);
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut();
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
      } => {
        assert_eq!(changed_row_id, row_id);
        assert_eq!(value, 120);
      },
      other => panic!("unexpected row change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut();
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
      } => {
        assert_eq!(changed_row_id, row_id);
        assert!(!value);
      },
      other => panic!("unexpected row change: {:?}", other),
    }
  }
}
