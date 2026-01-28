use crate::core::origin::CollabOrigin;
use crate::database::entity::DatabaseView;
use crate::database::views::define::*;
use crate::database::views::{
  DatabaseLayout, FieldOrder, FilterMap, GroupMap, RowOrder, SortMap, field_order_from_value,
  row_order_from_value, view_from_map_ref, view_from_value, view_id_from_map_ref,
};
use crate::preclude::array::ArrayEvent;
use crate::preclude::map::MapEvent;
use crate::preclude::{Change, MapRef, Subscription, ToJson, TransactionMut};
use crate::preclude::{DeepObservable, EntryChange, Event, PathSegment};
use crate::util::AnyExt;
use std::ops::Deref;
use std::str::FromStr;
use tokio::sync::broadcast;
use tracing::{trace, warn};

#[derive(Debug, Clone)]
pub enum DatabaseViewChange {
  DidCreateView {
    view: DatabaseView,
  },
  DidUpdateView {
    view: DatabaseView,
  },
  DidDeleteView {
    view_id: String,
  },
  LayoutSettingChanged {
    view_id: String,
    layout_type: DatabaseLayout,
    is_local_change: bool,
  },
  DidUpdateRowOrders {
    database_view_id: String,
    is_local_change: bool,
    insert_row_orders: Vec<(RowOrder, u32)>,
    delete_row_indexes: Vec<u32>,
  },
  // filter
  DidCreateFilters {
    view_id: String,
    is_local_change: bool,
    filters: Vec<FilterMap>,
  },
  DidUpdateFilter {
    view_id: String,
    is_local_change: bool,
  },
  // group
  DidCreateGroupSettings {
    view_id: String,
    is_local_change: bool,
    groups: Vec<GroupMap>,
  },
  DidUpdateGroupSetting {
    view_id: String,
    is_local_change: bool,
  },
  // Sort
  DidCreateSorts {
    view_id: String,
    is_local_change: bool,
    sorts: Vec<SortMap>,
  },
  DidUpdateSort {
    view_id: String,
    is_local_change: bool,
  },
  DidUpdateFieldOrders {
    view_id: String,
    is_local_change: bool,
    insert_field_orders: Vec<(FieldOrder, u32)>,
    delete_field_indexes: Vec<u32>,
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

pub(crate) fn subscribe_view_map_change(
  origin: CollabOrigin,
  view_map: &MapRef,
  change_tx: ViewChangeSender,
) -> Subscription {
  view_map.observe_deep(move |txn, events| {
    let txn_origin = CollabOrigin::from(txn);
    let is_local = txn_origin == origin;
    for event in events.iter() {
      match event {
        Event::Text(_) => {},
        Event::Array(array_event) => {
          handle_array_event(&change_tx, txn, array_event, is_local);
        },
        Event::Map(event) => {
          handle_map_event(&change_tx, txn, event, is_local);
        },
        _ => {},
      }
    }
  })
}

/// Handles an array modification process consisting of retain and remove operations.
///
/// # Process
/// 1. Initial Array State:
///    - Starts with the array `[A B C]`.
///    - Offset is initially at position 0.
///
/// 2. Retain Operation:
///    - Retain 1: Retains the first element (`A`), moving the offset to the next element.
///    - After operation: `[A B C]`
///    - Offset is now at position 1 (pointing to `B`).
///
/// 3. Remove Operation:
///    - Remove 1: Removes one element at the current offset.
///    - `B` (at offset position 1) is removed from the array.
///    - After operation: `[A   C]`
///    - Offset remains at position 1.
///
/// 4. Final Array State:
///    - Resulting array after the remove operation: `[A C]`
///    - This reflects the removal of `B` from the original array.
fn handle_array_event(
  change_tx: &ViewChangeSender,
  txn: &TransactionMut,
  array_event: &ArrayEvent,
  is_local_change: bool,
) {
  let mut offset = 0;
  let key = ArrayChangeKey::from(array_event);
  let mut delete_row_indexes: Vec<u32> = vec![];
  let mut insert_row_orders: Vec<(RowOrder, u32)> = vec![];
  let mut delete_field_indexes: Vec<u32> = vec![];
  let mut insert_field_orders: Vec<(FieldOrder, u32)> = vec![];
  if let Some(PathSegment::Key(view_id)) = array_event.path().front() {
    let database_view_id = view_id.to_string();
    array_event.delta(txn).iter().for_each(|change| {
      #[cfg(feature = "verbose_log")]
      trace!("database view observe array event: {:?}:{:?}", key, change);

      match change {
        Change::Added(values) => match &key {
          ArrayChangeKey::RowOrder => {
            let row_orders = values
              .iter()
              .flat_map(|value| {
                let value = row_order_from_value(value, txn).map(|row_order| (row_order, offset));
                offset += 1;
                value
              })
              .collect::<Vec<_>>();
            insert_row_orders.extend(row_orders.clone());
          },
          ArrayChangeKey::Filter => {
            if let Some(view_id) = view_id_from_array_event(array_event) {
              let filters: Vec<_> = values
                .iter()
                .flat_map(|value| value.to_json(txn).into_map())
                .collect();
              let _ = change_tx.send(DatabaseViewChange::DidCreateFilters {
                view_id,
                is_local_change,
                filters,
              });
            }
          },
          ArrayChangeKey::Sort => {
            if let Some(view_id) = view_id_from_array_event(array_event) {
              let sorts: Vec<_> = values
                .iter()
                .flat_map(|value| value.to_json(txn).into_map())
                .collect();
              let _ = change_tx.send(DatabaseViewChange::DidCreateSorts {
                view_id,
                is_local_change,
                sorts,
              });
            }
          },
          ArrayChangeKey::Group => {
            if let Some(view_id) = view_id_from_array_event(array_event) {
              let groups = values
                .iter()
                .flat_map(|value| value.to_json(txn).into_map())
                .collect::<Vec<_>>();
              let _ = change_tx.send(DatabaseViewChange::DidCreateGroupSettings {
                view_id,
                is_local_change,
                groups,
              });
            }
          },
          ArrayChangeKey::FieldOrder => {
            let field_orders = values
              .iter()
              .flat_map(|value| {
                let value =
                  field_order_from_value(value, txn).map(|field_order| (field_order, offset));
                offset += 1;
                value
              })
              .collect::<Vec<_>>();
            insert_field_orders.extend(field_orders.clone());
          },
          ArrayChangeKey::Unhandled(s) => {
            trace!("database view observe unknown insert: {}", s);
          },
        },
        Change::Removed(len) => {
          // https://github.com/y-crdt/y-crdt/issues/341
          #[cfg(feature = "verbose_log")]
          trace!("database view observe array remove: {}", len);
          match &key {
            ArrayChangeKey::RowOrder => {
              if *len > 0 {
                delete_row_indexes.extend((offset..=(offset + len - 1)).collect::<Vec<_>>());
              }
              offset += len;
            },
            ArrayChangeKey::Filter => {
              if let Some(view_id) = view_id_from_array_event(array_event) {
                let _ = change_tx.send(DatabaseViewChange::DidUpdateFilter {
                  view_id,
                  is_local_change,
                });
              }
            },
            ArrayChangeKey::Sort => {
              if let Some(view_id) = view_id_from_array_event(array_event) {
                let _ = change_tx.send(DatabaseViewChange::DidUpdateSort {
                  view_id,
                  is_local_change,
                });
              }
            },
            ArrayChangeKey::Group => {
              if let Some(view_id) = view_id_from_array_event(array_event) {
                let _ = change_tx.send(DatabaseViewChange::DidUpdateGroupSetting {
                  view_id,
                  is_local_change,
                });
              }
            },
            ArrayChangeKey::FieldOrder => {
              if *len > 0 {
                delete_field_indexes.extend((offset..=(offset + len - 1)).collect::<Vec<_>>());
              }
              offset += len;
            },
            ArrayChangeKey::Unhandled(_s) => {
              #[cfg(feature = "verbose_log")]
              trace!("database view observe unknown remove: {}", _s);
            },
          }
        },
        Change::Retain(value) => {
          offset += value;
          #[cfg(feature = "verbose_log")]
          trace!("database view observe array retain: {}", value);
        },
      }
    });

    let has_row_order_change = !insert_row_orders.is_empty() || !delete_row_indexes.is_empty();
    let has_field_order_change =
      !insert_field_orders.is_empty() || !delete_field_indexes.is_empty();

    if has_row_order_change {
      let _ = change_tx.send(DatabaseViewChange::DidUpdateRowOrders {
        database_view_id: database_view_id.clone(),
        is_local_change,
        insert_row_orders,
        delete_row_indexes,
      });
    }

    if has_field_order_change {
      let _ = change_tx.send(DatabaseViewChange::DidUpdateFieldOrders {
        view_id: database_view_id,
        is_local_change,
        insert_field_orders,
        delete_field_indexes,
      });
    }

    if !has_row_order_change && !has_field_order_change {
      #[cfg(feature = "verbose_log")]
      trace!("database view observe array event: no row/field order change");
    }
  } else {
    #[cfg(feature = "verbose_log")]
    trace!(
      "Can not find database view id when receive key:{:?} event:{:?}",
      key,
      array_event.path()
    );
  }
}

fn handle_map_event(
  change_tx: &ViewChangeSender,
  txn: &TransactionMut,
  event: &MapEvent,
  is_local_change: bool,
) {
  let path = event.path();
  let view_id_from_path = path.front().and_then(|segment| match segment {
    PathSegment::Key(key) => Some(key.to_string()),
    _ => None,
  });

  if path.len() > 1 {
    if let Some(view_id) = view_id_from_path {
      if path.iter().any(
        |segment| matches!(segment, PathSegment::Key(key) if key.as_ref() == DATABASE_VIEW_FILTERS),
      ) {
        let _ = change_tx.send(DatabaseViewChange::DidUpdateFilter {
          view_id,
          is_local_change,
        });
        return;
      }

      if path.iter().any(
        |segment| matches!(segment, PathSegment::Key(key) if key.as_ref() == DATABASE_VIEW_SORTS),
      ) {
        let _ = change_tx.send(DatabaseViewChange::DidUpdateSort {
          view_id,
          is_local_change,
        });
        return;
      }

      if path.iter().any(
        |segment| matches!(segment, PathSegment::Key(key) if key.as_ref() == DATABASE_VIEW_GROUPS),
      ) {
        let _ = change_tx.send(DatabaseViewChange::DidUpdateGroupSetting {
          view_id,
          is_local_change,
        });
      }
    }
    return;
  }

  let keys = event.keys(txn);
  for (key, value) in keys.iter() {
    let _change_tx = change_tx.clone();
    match value {
      EntryChange::Inserted(value) => {
        let database_view = view_from_value(value.clone(), txn);
        // trace!("database view map inserted: {}:{:?}", key, database_view,);
        if let Some(database_view) = database_view {
          let _ = change_tx.send(DatabaseViewChange::DidCreateView {
            view: database_view,
          });
        }
      },
      EntryChange::Updated(_, value) => {
        let database_view = view_from_map_ref(event.target(), txn);
        if let Some(database_view) = database_view {
          let _ = change_tx.send(DatabaseViewChange::DidUpdateView {
            view: database_view,
          });
        }

        let view_id = view_id_from_map_ref(event.target(), txn);
        trace!("database view map update: {}:{}", key, value);
        match (*key).as_ref() {
          DATABASE_VIEW_LAYOUT => {
            if let Ok(layout_type) = DatabaseLayout::from_str(&value.to_string()) {
              let _ = change_tx.send(DatabaseViewChange::LayoutSettingChanged {
                view_id,
                layout_type,
                is_local_change,
              });
            }
          },
          _ => {
            trace!("database view map update: {}:{}", key, value);
          },
        }
      },
      EntryChange::Removed(_value) => {
        // trace!("database view map delete: {}:{}", key, value);
        let view_id = (**key).to_string();
        if !view_id.is_empty() {
          let _ = change_tx.send(DatabaseViewChange::DidDeleteView { view_id });
        } else {
          warn!("database view map delete: empty key");
        }
      },
    }
  }
}

#[derive(Debug)]
enum ArrayChangeKey {
  Unhandled(String),
  RowOrder,
  FieldOrder,
  Filter,
  Sort,
  Group,
}

impl From<&ArrayEvent> for ArrayChangeKey {
  fn from(event: &ArrayEvent) -> Self {
    match event.path().pop_back() {
      Some(segment) => match segment {
        PathSegment::Key(s) => match s.as_ref() {
          DATABASE_VIEW_ROW_ORDERS => Self::RowOrder,
          DATABASE_VIEW_FIELD_ORDERS => Self::FieldOrder,
          DATABASE_VIEW_FILTERS => Self::Filter,
          DATABASE_VIEW_SORTS => Self::Sort,
          DATABASE_VIEW_GROUPS => Self::Group,
          _ => Self::Unhandled(s.deref().to_string()),
        },
        PathSegment::Index(_) => Self::Unhandled("index".to_string()),
      },
      None => Self::Unhandled("empty path".to_string()),
    }
  }
}

fn view_id_from_array_event(event: &ArrayEvent) -> Option<String> {
  let path = event.path();
  if path.len() > 1 {
    match path.front() {
      Some(PathSegment::Key(key)) => Some(key.to_string()),
      _ => None,
    }
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::origin::{CollabClient, CollabOrigin};
  use crate::database::views::{
    DatabaseLayout, FieldOrder, GroupSettingMap, RowOrder, SortMap, ViewBuilder,
  };
  use crate::entity::uuid_validation::RowId;
  use crate::preclude::{
    Any, Array, ArrayRef, Doc, Map, MapExt, MapPrelim, ReadTxn, Transact, TransactionMut,
  };
  use std::collections::HashMap;
  use std::time::Duration;
  use tokio::sync::broadcast;
  use tokio::time::timeout;
  use uuid::Uuid;

  const CHANGE_TIMEOUT: Duration = Duration::from_secs(2);

  async fn recv_with_timeout<T: Clone>(rx: &mut broadcast::Receiver<T>) -> T {
    timeout(CHANGE_TIMEOUT, rx.recv())
      .await
      .expect("timed out waiting for change")
      .expect("change channel closed unexpectedly")
  }

  fn drain<T: Clone>(rx: &mut broadcast::Receiver<T>) {
    loop {
      match rx.try_recv() {
        Ok(_) => continue,
        Err(broadcast::error::TryRecvError::Empty) => break,
        Err(broadcast::error::TryRecvError::Closed) => break,
        Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
      }
    }
  }

  async fn recv_until<F>(rx: &mut ViewChangeReceiver, mut predicate: F) -> DatabaseViewChange
  where
    F: FnMut(&DatabaseViewChange) -> bool,
  {
    for _ in 0..32 {
      let change = recv_with_timeout(rx).await;
      if predicate(&change) {
        return change;
      }
    }
    panic!("expected change not received");
  }

  fn local_and_remote_origins() -> (CollabOrigin, CollabOrigin) {
    let local = CollabOrigin::Client(CollabClient::new(0xdeadbeef, "local-device"));
    let remote = CollabOrigin::Client(CollabClient::new(0xfeedface, "remote-device"));
    (local, remote)
  }

  fn insert_basic_view(
    txn: &mut TransactionMut,
    views_map: &MapRef,
    view_id: &str,
    name: &str,
  ) -> MapRef {
    let map_ref = views_map.insert(txn, view_id, MapPrelim::default());
    ViewBuilder::new(txn, map_ref.clone())
      .update(|update| {
        update
          .set_view_id(view_id)
          .set_name(name)
          .set_layout_type(DatabaseLayout::Grid)
          // Initialize nested types so subsequent updates are observable.
          .set_filters(Vec::new())
          .set_sorts(Vec::new())
          .set_groups(Vec::new())
          .set_row_orders(Vec::new())
          .set_field_orders(Vec::new());
      })
      .done();
    map_ref
  }

  fn first_array_item_as_map_ref<T: ReadTxn>(array_ref: &ArrayRef, txn: &T) -> MapRef {
    let value = array_ref
      .get(txn, 0)
      .expect("expected array element at index 0");
    value
      .cast::<MapRef>()
      .expect("expected array element to be a MapRef")
  }

  #[tokio::test]
  async fn view_create_update_delete_emits_events() {
    let doc = Doc::new();
    let views_map: MapRef = doc.get_or_insert_map("views");
    let (origin, _) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(128);
    let _sub = subscribe_view_map_change(origin.clone(), &views_map, change_tx);

    let view_uuid = Uuid::new_v4();
    let view_id = view_uuid.to_string();

    {
      let mut txn = doc.transact_mut_with(origin.clone());
      insert_basic_view(&mut txn, &views_map, &view_id, "View");
    }

    let created = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidCreateView { .. })
    })
    .await;
    match created {
      DatabaseViewChange::DidCreateView { view } => {
        assert_eq!(view.id, view_uuid);
        assert_eq!(view.name, "View");
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_name("Renamed");
        })
        .done();
    }

    let updated = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateView { .. })
    })
    .await;
    match updated {
      DatabaseViewChange::DidUpdateView { view } => assert_eq!(view.name, "Renamed"),
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      views_map.remove(&mut txn, &view_id);
    }

    let deleted = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidDeleteView { .. })
    })
    .await;
    match deleted {
      DatabaseViewChange::DidDeleteView { view_id: deleted } => assert_eq!(deleted, view_id),
      other => panic!("unexpected change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn layout_setting_change_marks_local_and_remote() {
    let doc = Doc::new();
    let views_map: MapRef = doc.get_or_insert_map("views");
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(128);
    let _sub = subscribe_view_map_change(origin.clone(), &views_map, change_tx);

    let view_id = Uuid::new_v4().to_string();
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      insert_basic_view(&mut txn, &views_map, &view_id, "View");
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_layout_type(DatabaseLayout::Board);
        })
        .done();
    }

    let local_change = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::LayoutSettingChanged { .. })
    })
    .await;
    match local_change {
      DatabaseViewChange::LayoutSettingChanged {
        view_id: changed_view_id,
        layout_type,
        is_local_change,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert_eq!(layout_type, DatabaseLayout::Board);
        assert!(is_local_change);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin);
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_layout_type(DatabaseLayout::Calendar);
        })
        .done();
    }

    let remote_change = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::LayoutSettingChanged { .. })
    })
    .await;
    match remote_change {
      DatabaseViewChange::LayoutSettingChanged {
        view_id: changed_view_id,
        layout_type,
        is_local_change,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert_eq!(layout_type, DatabaseLayout::Calendar);
        assert!(!is_local_change);
      },
      other => panic!("unexpected change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn row_orders_emit_insert_delete_move_and_locality() {
    let doc = Doc::new();
    let views_map: MapRef = doc.get_or_insert_map("views");
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    let _sub = subscribe_view_map_change(origin.clone(), &views_map, change_tx);

    let view_id = Uuid::new_v4().to_string();
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      insert_basic_view(&mut txn, &views_map, &view_id, "View");
    }

    let row1 = RowOrder::new(RowId::new_v4(), 0);
    let row2 = RowOrder::new(RowId::new_v4(), 0);
    let row3 = RowOrder::new(RowId::new_v4(), 0);

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_row_orders(vec![row1.clone(), row2.clone(), row3.clone()]);
        })
        .done();
    }

    let inserted = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateRowOrders { .. })
    })
    .await;
    match inserted {
      DatabaseViewChange::DidUpdateRowOrders {
        database_view_id,
        is_local_change,
        insert_row_orders,
        delete_row_indexes,
      } => {
        assert_eq!(database_view_id, view_id);
        assert!(!is_local_change);
        assert_eq!(delete_row_indexes, Vec::<u32>::new());
        assert_eq!(insert_row_orders.len(), 3);
        assert_eq!(insert_row_orders[0].0.id, row1.id);
        assert_eq!(insert_row_orders[0].1, 0);
        assert_eq!(insert_row_orders[1].0.id, row2.id);
        assert_eq!(insert_row_orders[1].1, 1);
        assert_eq!(insert_row_orders[2].0.id, row3.id);
        assert_eq!(insert_row_orders[2].1, 2);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.remove_row_order(&row2.id.to_string());
        })
        .done();
    }

    let deleted = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateRowOrders { .. })
    })
    .await;
    match deleted {
      DatabaseViewChange::DidUpdateRowOrders {
        database_view_id,
        is_local_change,
        insert_row_orders,
        delete_row_indexes,
      } => {
        assert_eq!(database_view_id, view_id);
        assert!(!is_local_change);
        assert!(insert_row_orders.is_empty());
        assert_eq!(delete_row_indexes, vec![1]);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.move_row_order(&row1.id.to_string(), &row3.id.to_string());
        })
        .done();
    }

    let moved = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateRowOrders { .. })
    })
    .await;
    match moved {
      DatabaseViewChange::DidUpdateRowOrders {
        database_view_id,
        is_local_change,
        insert_row_orders,
        delete_row_indexes,
      } => {
        assert_eq!(database_view_id, view_id);
        assert!(!is_local_change);
        assert!(!insert_row_orders.is_empty());
        assert!(!delete_row_indexes.is_empty());
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.insert_row_order(
            RowOrder::new(RowId::new_v4(), 0),
            &crate::database::views::OrderObjectPosition::End,
          );
        })
        .done();
    }

    let local_insert = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateRowOrders { .. })
    })
    .await;
    match local_insert {
      DatabaseViewChange::DidUpdateRowOrders {
        database_view_id,
        is_local_change,
        insert_row_orders,
        ..
      } => {
        assert_eq!(database_view_id, view_id);
        assert!(is_local_change);
        assert_eq!(insert_row_orders.len(), 1);
      },
      other => panic!("unexpected change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn field_orders_emit_insert_delete_move_and_locality() {
    let doc = Doc::new();
    let views_map: MapRef = doc.get_or_insert_map("views");
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    let _sub = subscribe_view_map_change(origin.clone(), &views_map, change_tx);

    let view_id = Uuid::new_v4().to_string();
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      insert_basic_view(&mut txn, &views_map, &view_id, "View");
    }

    let field1 = FieldOrder::new(Uuid::new_v4().to_string());
    let field2 = FieldOrder::new(Uuid::new_v4().to_string());
    let field3 = FieldOrder::new(Uuid::new_v4().to_string());

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_field_orders(vec![field1.clone(), field2.clone(), field3.clone()]);
        })
        .done();
    }

    let inserted = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateFieldOrders { .. })
    })
    .await;
    match inserted {
      DatabaseViewChange::DidUpdateFieldOrders {
        view_id: changed_view_id,
        is_local_change,
        insert_field_orders,
        delete_field_indexes,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert_eq!(delete_field_indexes, Vec::<u32>::new());
        assert_eq!(insert_field_orders.len(), 3);
        assert_eq!(insert_field_orders[0].0.id, field1.id);
        assert_eq!(insert_field_orders[0].1, 0);
        assert_eq!(insert_field_orders[1].0.id, field2.id);
        assert_eq!(insert_field_orders[1].1, 1);
        assert_eq!(insert_field_orders[2].0.id, field3.id);
        assert_eq!(insert_field_orders[2].1, 2);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.remove_field_order(&field2.id);
        })
        .done();
    }

    let deleted = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateFieldOrders { .. })
    })
    .await;
    match deleted {
      DatabaseViewChange::DidUpdateFieldOrders {
        view_id: changed_view_id,
        is_local_change,
        insert_field_orders,
        delete_field_indexes,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert!(insert_field_orders.is_empty());
        assert_eq!(delete_field_indexes, vec![1]);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.move_field_order(&field1.id, &field3.id);
        })
        .done();
    }

    let moved = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateFieldOrders { .. })
    })
    .await;
    match moved {
      DatabaseViewChange::DidUpdateFieldOrders {
        view_id: changed_view_id,
        is_local_change,
        insert_field_orders,
        delete_field_indexes,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert!(!insert_field_orders.is_empty());
        assert!(!delete_field_indexes.is_empty());
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.insert_field_order(
            FieldOrder::new(Uuid::new_v4().to_string()),
            &crate::database::views::OrderObjectPosition::End,
          );
        })
        .done();
    }

    let local_insert = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateFieldOrders { .. })
    })
    .await;
    match local_insert {
      DatabaseViewChange::DidUpdateFieldOrders {
        view_id: changed_view_id,
        is_local_change,
        insert_field_orders,
        ..
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(is_local_change);
        assert_eq!(insert_field_orders.len(), 1);
      },
      other => panic!("unexpected change: {:?}", other),
    }
  }

  #[tokio::test]
  async fn filters_sorts_groups_emit_create_and_deep_update_for_remote_changes() {
    let doc = Doc::new();
    let views_map: MapRef = doc.get_or_insert_map("views");
    let (origin, remote_origin) = local_and_remote_origins();

    let (change_tx, mut change_rx) = broadcast::channel(256);
    let _sub = subscribe_view_map_change(origin.clone(), &views_map, change_tx);

    let view_id = Uuid::new_v4().to_string();
    {
      let mut txn = doc.transact_mut_with(origin.clone());
      insert_basic_view(&mut txn, &views_map, &view_id, "View");
    }

    let filter: FilterMap = HashMap::from([("id".into(), Any::from("filter-1"))]);

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_filters(vec![filter]);
        })
        .done();
    }

    let filter_created = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidCreateFilters { .. })
    })
    .await;
    match filter_created {
      DatabaseViewChange::DidCreateFilters {
        view_id: changed_view_id,
        is_local_change,
        filters,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert_eq!(filters.len(), 1);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.update_filters(|txn: &mut TransactionMut, array_ref| {
            let map_ref = first_array_item_as_map_ref(&array_ref, txn);
            map_ref.insert(txn, "content", "changed");
          });
        })
        .done();
    }

    let filter_updated = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateFilter { .. })
    })
    .await;
    match filter_updated {
      DatabaseViewChange::DidUpdateFilter {
        view_id: changed_view_id,
        is_local_change,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    let sort: SortMap = HashMap::from([("id".into(), Any::from("sort-1"))]);

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_sorts(vec![sort]);
        })
        .done();
    }

    let sort_created = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidCreateSorts { .. })
    })
    .await;
    match sort_created {
      DatabaseViewChange::DidCreateSorts {
        view_id: changed_view_id,
        is_local_change,
        sorts,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert_eq!(sorts.len(), 1);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.update_sorts(|txn: &mut TransactionMut, array_ref| {
            let map_ref = first_array_item_as_map_ref(&array_ref, txn);
            map_ref.insert(txn, "content", "changed");
          });
        })
        .done();
    }

    let sort_updated = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateSort { .. })
    })
    .await;
    match sort_updated {
      DatabaseViewChange::DidUpdateSort {
        view_id: changed_view_id,
        is_local_change,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    let group: GroupSettingMap = HashMap::from([("id".into(), Any::from("group-1"))]);

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin.clone());
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.set_groups(vec![group]);
        })
        .done();
    }

    let group_created = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidCreateGroupSettings { .. })
    })
    .await;
    match group_created {
      DatabaseViewChange::DidCreateGroupSettings {
        view_id: changed_view_id,
        is_local_change,
        groups,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
        assert_eq!(groups.len(), 1);
      },
      other => panic!("unexpected change: {:?}", other),
    }

    drain(&mut change_rx);
    {
      let mut txn = doc.transact_mut_with(remote_origin);
      let view_map: MapRef = views_map
        .get_with_txn(&txn, &view_id)
        .expect("missing view map");
      ViewBuilder::new(&mut txn, view_map)
        .update(|update| {
          update.update_groups(|txn: &mut TransactionMut, array_ref| {
            let map_ref = first_array_item_as_map_ref(&array_ref, txn);
            map_ref.insert(txn, "content", "changed");
          });
        })
        .done();
    }

    let group_updated = recv_until(&mut change_rx, |change| {
      matches!(change, DatabaseViewChange::DidUpdateGroupSetting { .. })
    })
    .await;
    match group_updated {
      DatabaseViewChange::DidUpdateGroupSetting {
        view_id: changed_view_id,
        is_local_change,
      } => {
        assert_eq!(changed_view_id, view_id);
        assert!(!is_local_change);
      },
      other => panic!("unexpected change: {:?}", other),
    }
  }
}
