use collab::preclude::{
  Array, ArrayRef, Map, MapExt, MapPrelim, MapRef, ReadTxn, Subscription, TransactionMut,
};

use crate::database::timestamp;
use crate::entity::{DatabaseView, DatabaseViewMeta};
use crate::rows::RowId;
use crate::views::define::*;
use crate::views::{
  CalculationMap, DatabaseLayout, DatabaseViewUpdate, FieldOrder, FieldOrderArray,
  FieldSettingsByFieldIdMap, FilterMap, GroupSettingMap, LayoutSetting, OrderArray, RowOrder,
  RowOrderArray, SortMap, ViewBuilder, ViewChangeSender, field_settings_from_map_ref,
  filters_from_map_ref, group_setting_from_map_ref, layout_setting_from_map_ref,
  sorts_from_map_ref, subscribe_view_map_change, view_from_map_ref, view_from_value,
  view_meta_from_value,
};
use collab::core::origin::CollabOrigin;
use std::ops::Deref;

use super::{calculations_from_map_ref, view_id_from_map_ref};

/// `ViewMap` manages views within a database.
///
/// This class provides methods to insert, update, delete, and retrieve views. Each view is stored
/// as a key/value pair within the `ViewMap`. The key is the view ID, and the value is the view data.
///
/// ## Structure of View Data
/// The view data is organized in JSON format, where each view is identified by a unique view ID.
/// Below is an example of how the views are stored:
///
/// ```json
/// {
///     "view_id_1": "view_data",
///     "view_id_2": "view_data",
///     "view_id_3": "view_data"
/// }
/// Each view data can be deserialize into a `DatabaseView` struct.
///
pub struct DatabaseViews {
  container: MapRef,
  #[allow(dead_code)]
  view_map_subscription: Option<Subscription>,
}

impl Deref for DatabaseViews {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}

impl DatabaseViews {
  pub fn new(
    origin: CollabOrigin,
    container: MapRef,
    view_change_sender: Option<ViewChangeSender>,
  ) -> Self {
    let view_map_subscription = view_change_sender
      .map(|sender| subscribe_view_map_change(origin, &container, sender.clone()));
    Self {
      container,
      view_map_subscription,
    }
  }

  pub fn insert_view(&self, txn: &mut TransactionMut, view: DatabaseView) {
    let map_ref = self
      .container
      .insert(txn, view.id.as_str(), MapPrelim::default());
    ViewBuilder::new(txn, map_ref).update(|update| {
      update
        .set_view_id(&view.id)
        .set_database_id(view.database_id)
        .set_name(view.name)
        .set_created_at(view.created_at)
        .set_modified_at(view.modified_at)
        .set_layout_settings(view.layout_settings)
        .set_layout_type(view.layout)
        .set_field_settings(view.field_settings)
        .set_filters(view.filters)
        .set_groups(view.group_settings)
        .set_sorts(view.sorts)
        .set_field_orders(view.field_orders)
        .set_row_orders(view.row_orders)
        .set_is_inline(view.is_inline);
    });
  }

  pub fn get_view_group_setting<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<GroupSettingMap> {
    if let Some(map_ref) = self.container.get_with_txn(txn, view_id) {
      group_setting_from_map_ref(txn, &map_ref)
    } else {
      vec![]
    }
  }

  pub fn get_view_sorts<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<SortMap> {
    if let Some(map_ref) = self.container.get_with_txn(txn, view_id) {
      sorts_from_map_ref(txn, &map_ref)
    } else {
      vec![]
    }
  }

  pub fn get_view_calculations<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<CalculationMap> {
    if let Some(map_ref) = self.container.get_with_txn(txn, view_id) {
      calculations_from_map_ref(txn, &map_ref)
    } else {
      vec![]
    }
  }

  pub fn get_view_filters<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<FilterMap> {
    if let Some(map_ref) = self.container.get_with_txn(txn, view_id) {
      filters_from_map_ref(txn, &map_ref)
    } else {
      vec![]
    }
  }

  pub fn get_layout_setting<T: ReadTxn, V: From<LayoutSetting>>(
    &self,
    txn: &T,
    view_id: &str,
    layout_ty: &DatabaseLayout,
  ) -> Option<V> {
    if let Some(map_ref) = self.container.get_with_txn(txn, view_id) {
      layout_setting_from_map_ref(txn, &map_ref)
        .get(layout_ty)
        .map(|value| V::from(value.clone()))
    } else {
      None
    }
  }

  pub fn get_view_field_settings<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
  ) -> FieldSettingsByFieldIdMap {
    self
      .container
      .get_with_txn(txn, view_id)
      .map(|map_ref| field_settings_from_map_ref(txn, &map_ref))
      .unwrap_or_default()
  }

  pub fn get_view<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<DatabaseView> {
    let map_ref = self.container.get_with_txn(txn, view_id)?;
    view_from_map_ref(&map_ref, txn)
  }

  pub fn get_all_views<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseView> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| view_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn get_all_views_meta<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseViewMeta> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| view_meta_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn get_database_view_layout<T: ReadTxn>(&self, txn: &T, view_id: &str) -> DatabaseLayout {
    let layout_type = self
      .container
      .get_with_txn::<_, MapRef>(txn, view_id)
      .map(|map_ref| {
        map_ref
          .get_with_txn::<_, i64>(txn, DATABASE_VIEW_LAYOUT)
          .map(DatabaseLayout::from)
      });

    match layout_type {
      Some(Some(layout_type)) => layout_type,
      _ => DatabaseLayout::Grid,
    }
  }

  pub fn get_row_order_at_index<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    index: u32,
  ) -> Option<RowOrder> {
    self
      .container
      .get_with_txn::<_, MapRef>(txn, view_id)
      .and_then(|map_ref| {
        map_ref
          .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_ROW_ORDERS)
          .map(|array_ref| RowOrderArray::new(array_ref).get_object_at_index(txn, index))
      })?
  }

  pub fn get_row_orders<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<RowOrder> {
    self
      .container
      .get_with_txn::<_, MapRef>(txn, view_id)
      .map(|map_ref| {
        map_ref
          .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_ROW_ORDERS)
          .map(|array_ref| RowOrderArray::new(array_ref).get_objects_with_txn(txn))
          .unwrap_or_default()
      })
      .unwrap_or_default()
  }

  pub fn update_row_orders_with_txn<F>(&self, txn: &mut TransactionMut, view_id: &str, f: &mut F)
  where
    F: FnMut(&mut RowOrder),
  {
    if let Some(row_order_map) = self
      .container
      .get_with_txn::<_, MapRef>(txn, view_id)
      .and_then(|map_ref| map_ref.get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_ROW_ORDERS))
    {
      let row_order_array = RowOrderArray::new(row_order_map);
      for mut row_order in row_order_array.get_objects_with_txn(txn) {
        row_order_array.remove_with_txn(txn, row_order.id.as_str());
        f(&mut row_order);
        row_order_array.push_back(txn, row_order);
      }
    }
  }

  pub fn get_row_index<T: ReadTxn>(&self, txn: &T, view_id: &str, row_id: &RowId) -> Option<u32> {
    let map: MapRef = self.container.get_with_txn(txn, view_id)?;
    let row_order_array: ArrayRef = map.get_with_txn(txn, DATABASE_VIEW_ROW_ORDERS)?;
    RowOrderArray::new(row_order_array).get_position_with_txn(txn, row_id.as_str())
  }

  pub fn get_field_orders<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<FieldOrder> {
    self
      .container
      .get_with_txn::<_, MapRef>(txn, view_id)
      .map(|map_ref| {
        map_ref
          .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_FIELD_ORDERS)
          .map(|array_ref| FieldOrderArray::new(array_ref).get_objects_with_txn(txn))
          .unwrap_or_default()
      })
      .unwrap_or_default()
  }

  pub fn update_database_view<F>(&self, txn: &mut TransactionMut, view_id: &str, f: F)
  where
    F: FnOnce(DatabaseViewUpdate),
  {
    if let Some(map_ref) = self.container.get_with_txn::<_, MapRef>(txn, view_id) {
      let mut update = DatabaseViewUpdate::new(txn, &map_ref);
      update = update.set_modified_at(timestamp());
      f(update)
    } else {
      tracing::error!(
        "Can't update the database view:{}. The view is not found",
        view_id
      )
    }
  }

  pub fn update_all_views<F>(&self, txn: &mut TransactionMut, f: F)
  where
    F: Fn(String, DatabaseViewUpdate),
  {
    let map_refs: Vec<_> = self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| v.cast::<MapRef>().ok())
      .collect();

    for map_ref in map_refs {
      let view_id = view_id_from_map_ref(&map_ref, txn);
      let mut update = DatabaseViewUpdate::new(txn, &map_ref);
      update = update.set_modified_at(timestamp());
      f(view_id, update)
    }
  }

  pub fn clear(&self, txn: &mut TransactionMut) {
    self.container.clear(txn);
  }

  pub fn delete_view(&self, txn: &mut TransactionMut, view_id: &str) {
    self.container.remove(txn, view_id);
  }
}
