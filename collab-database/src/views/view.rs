use collab::preclude::{
  Any, Array, ArrayRef, FillRef, Map, MapExt, MapRef, ReadTxn, ToJson, TransactionMut, YrsValue,
};
use collab::util::AnyExt;
use std::collections::HashMap;
use tracing::trace;

use super::CalculationMap;

use crate::entity::{DatabaseView, DatabaseViewMeta};

use crate::views::define::*;
use crate::views::layout::{DatabaseLayout, LayoutSettings};
use crate::views::{
  FieldOrder, FieldOrderArray, FieldSettingsByFieldIdMap, FilterArray, FilterMap,
  GroupSettingArray, GroupSettingMap, LayoutSetting, RowOrder, RowOrderArray, SortArray, SortMap,
};
use crate::{impl_any_update, impl_i64_update, impl_order_update, impl_str_update};

pub struct ViewBuilder<'a, 'b> {
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(DatabaseViewUpdate),
  {
    let update = DatabaseViewUpdate::new(self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

#[derive(Debug, Default, Clone)]
pub enum OrderObjectPosition {
  Start,
  Before(String),
  After(String),
  #[default]
  End,
}

pub struct DatabaseViewUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> DatabaseViewUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn set_view_id(self, view_id: &str) -> Self {
    self.map_ref.insert(self.txn, VIEW_ID, view_id);
    self
  }

  pub fn set_is_inline(self, is_inline: bool) -> Self {
    self.map_ref.insert(self.txn, IS_INLINE, is_inline);
    self
  }

  impl_str_update!(
    set_database_id,
    set_database_id_if_not_none,
    VIEW_DATABASE_ID
  );

  impl_i64_update!(set_created_at, set_created_at_if_not_none, VIEW_CREATE_AT);
  impl_i64_update!(set_modified_at, set_modified_at_if_not_none, VIEW_MODIFY_AT);
  impl_str_update!(set_name, set_name_if_not_none, VIEW_NAME);

  impl_any_update!(
    set_layout_type,
    set_layout_type_if_not_none,
    DATABASE_VIEW_LAYOUT,
    DatabaseLayout
  );

  impl_order_update!(
    set_row_orders,
    remove_row_order,
    move_row_order,
    insert_row_order,
    iter_mut_row_order,
    DATABASE_VIEW_ROW_ORDERS,
    RowOrder,
    RowOrderArray
  );

  impl_order_update!(
    set_field_orders,
    remove_field_order,
    move_field_order,
    insert_field_order,
    iter_mut_field_order,
    DATABASE_VIEW_FIELD_ORDERS,
    FieldOrder,
    FieldOrderArray
  );

  /// Set layout settings of the current view
  pub fn set_layout_settings(self, layout_settings: LayoutSettings) -> Self {
    let map_ref: MapRef = self.map_ref.get_or_init(self.txn, VIEW_LAYOUT_SETTINGS);
    layout_settings.fill_map_ref(self.txn, &map_ref);
    self
  }

  /// Update layout setting for the given [DatabaseLayout]
  /// If the layout setting is not exist, it will be created
  /// If the layout setting is exist, it will be updated
  pub fn update_layout_settings(
    self,
    layout_ty: &DatabaseLayout,
    layout_setting: LayoutSetting,
  ) -> Self {
    let layout_settings: MapRef = self.map_ref.get_or_init(self.txn, VIEW_LAYOUT_SETTINGS);
    let layout_setting_map: MapRef = layout_settings.get_or_init(self.txn, layout_ty.as_ref());
    Any::from(layout_setting)
      .fill(self.txn, &layout_setting_map)
      .unwrap();
    self
  }

  /// Remove layout setting for the given [DatabaseLayout]
  pub fn remove_layout_setting(self, layout_ty: &DatabaseLayout) -> Self {
    let layout_settings: MapRef = self.map_ref.get_or_init(self.txn, VIEW_LAYOUT_SETTINGS);
    layout_settings.remove(self.txn, layout_ty.as_ref());
    self
  }

  /// Update calculations
  pub fn update_calculations<F>(mut self, f: F) -> Self
  where
    F: FnOnce(&mut TransactionMut, ArrayRef),
  {
    let array_ref = self.get_calculations_array();
    f(self.txn, array_ref);
    self
  }

  fn get_calculations_array(&mut self) -> ArrayRef {
    self.map_ref.get_or_init(self.txn, VIEW_CALCULATIONS)
  }

  /// Set filters of the current view
  pub fn set_filters(mut self, filters: Vec<FilterMap>) -> Self {
    let array_ref = self.get_filter_array();
    let filter_array: FilterArray = filters.into_iter().map(Any::from).collect();
    Any::from(filter_array).fill(self.txn, &array_ref).unwrap();
    self
  }

  /// Update filters
  /// The given function, [ArrayMapUpdate], which can be used to update the filters
  pub fn update_filters<F>(mut self, f: F) -> Self
  where
    F: FnOnce(&mut TransactionMut, ArrayRef),
  {
    let array_ref = self.get_filter_array();
    f(self.txn, array_ref);
    self
  }

  /// Set groups of the current view
  pub fn set_groups(mut self, group_settings: Vec<GroupSettingMap>) -> Self {
    let array_ref = self.get_group_array();
    let group_settings: GroupSettingArray = group_settings.into_iter().map(Any::from).collect();
    Any::from(group_settings)
      .fill(self.txn, &array_ref)
      .unwrap();
    self
  }

  /// Update groups
  /// The given function, [ArrayMapUpdate], which can be used to update the groups
  pub fn update_groups<F>(mut self, f: F) -> Self
  where
    F: FnOnce(&mut TransactionMut, ArrayRef),
  {
    let array_ref = self.get_group_array();
    f(self.txn, array_ref);
    self
  }

  /// Set sorts of the current view
  pub fn set_sorts(mut self, sorts: Vec<SortMap>) -> Self {
    let array_ref = self.get_sort_array();
    let sort_array: SortArray = sorts.into_iter().map(Any::from).collect();
    Any::from(sort_array).fill(self.txn, &array_ref).unwrap();
    self
  }

  /// Update sorts
  /// The given function, [ArrayMapUpdate], which can be used to update the sorts
  pub fn update_sorts<F>(mut self, f: F) -> Self
  where
    F: FnOnce(&mut TransactionMut, ArrayRef),
  {
    let array_ref = self.get_sort_array();
    f(self.txn, array_ref);
    self
  }

  /// Set the field settings of the current view
  pub fn set_field_settings(mut self, field_settings: FieldSettingsByFieldIdMap) -> Self {
    let map_ref = self.get_field_settings_map();
    field_settings.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn update_field_settings_for_fields<F>(mut self, field_ids: Vec<String>, f: F) -> Self
  where
    F: Fn(&mut TransactionMut, MapRef, &str, DatabaseLayout),
  {
    let map_ref = self.get_field_settings_map();
    let layout_ty = self.get_layout_setting().unwrap();
    field_ids.iter().for_each(|field_id| {
      f(self.txn, map_ref.clone(), field_id.as_str(), layout_ty);
    });
    self
  }

  pub fn remove_field_setting(mut self, field_id: &str) -> Self {
    let map_ref = self.get_field_settings_map();
    map_ref.remove(self.txn, field_id);
    self
  }

  /// Get the sort array for the current view, used when setting or updating
  /// sort array
  fn get_sort_array(&mut self) -> ArrayRef {
    self.map_ref.get_or_init(self.txn, DATABASE_VIEW_SORTS)
  }

  /// Get the group array for the current view, used when setting or updating
  /// group array
  fn get_group_array(&mut self) -> ArrayRef {
    self.map_ref.get_or_init(self.txn, DATABASE_VIEW_GROUPS)
  }

  /// Get the filter array for the current view, used when setting or updating
  /// filter array
  fn get_filter_array(&mut self) -> ArrayRef {
    self.map_ref.get_or_init(self.txn, DATABASE_VIEW_FILTERS)
  }

  /// Get the field settings for the current view, used when setting or updating
  /// field settings
  fn get_field_settings_map(&mut self) -> MapRef {
    self
      .map_ref
      .get_or_init(self.txn, DATABASE_VIEW_FIELD_SETTINGS)
  }

  fn get_layout_setting(&self) -> Option<DatabaseLayout> {
    self
      .map_ref
      .get_with_txn::<_, i64>(self.txn, DATABASE_VIEW_LAYOUT)
      .map(DatabaseLayout::from)
  }

  pub fn done(self) -> Option<DatabaseView> {
    view_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn view_id_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> String {
  map_ref.get_with_txn(txn, VIEW_ID).unwrap_or_default()
}
pub fn view_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref: MapRef = value.cast().ok()?;
  Some(view_id_from_map_ref(&map_ref, txn))
}

/// Return a [DatabaseViewMeta] from a map ref
/// A [DatabaseViewMeta] is a subset of a [DatabaseView]
pub fn view_meta_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<DatabaseViewMeta> {
  let map_ref: MapRef = value.cast().ok()?;
  let id: String = map_ref.get_with_txn(txn, VIEW_ID)?;
  let name: String = map_ref.get_with_txn(txn, VIEW_NAME).unwrap_or_default();
  let is_inline = map_ref.get_with_txn(txn, IS_INLINE).unwrap_or_default();
  Some(DatabaseViewMeta {
    id,
    name,
    is_inline,
  })
}

/// Return a [DatabaseView] from a map ref
pub fn view_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<DatabaseView> {
  let map_ref: MapRef = value.cast().ok()?;
  view_from_map_ref(&map_ref, txn)
}

fn array_of_maps(array_ref: ArrayRef, txn: &impl ReadTxn) -> Vec<HashMap<String, Any>> {
  array_ref
    .to_json(txn)
    .into_array()
    .unwrap()
    .into_iter()
    .map(|any| any.into_map().unwrap())
    .collect()
}

/// Return a list of [GroupSettingMap] from a map ref
pub fn group_setting_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<GroupSettingMap> {
  map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_GROUPS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default()
}

/// Return a new list of [SortMap]s from a map ref
pub fn sorts_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<SortMap> {
  map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_SORTS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default()
}

/// Return a new list of [CalculationMap]s from a map ref
pub fn calculations_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<CalculationMap> {
  map_ref
    .get_with_txn::<_, ArrayRef>(txn, VIEW_CALCULATIONS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default()
}

/// Return a new list of [FilterMap]s from a map ref
pub fn filters_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<FilterMap> {
  map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_FILTERS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default()
}

/// Creates a new layout settings from a map ref
pub fn layout_setting_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> LayoutSettings {
  map_ref
    .get_with_txn::<_, MapRef>(txn, VIEW_LAYOUT_SETTINGS)
    .map(|map_ref| LayoutSettings::from_map_ref(txn, map_ref))
    .unwrap_or_default()
}

/// Creates a new field settings from a map ref
pub fn field_settings_from_map_ref<T: ReadTxn>(
  txn: &T,
  map_ref: &MapRef,
) -> FieldSettingsByFieldIdMap {
  map_ref
    .get_with_txn::<_, MapRef>(txn, DATABASE_VIEW_FIELD_SETTINGS)
    .map(|map_ref| FieldSettingsByFieldIdMap::from((txn, &map_ref)))
    .unwrap_or_default()
}

/// Creates a new view from a map ref
pub fn view_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<DatabaseView> {
  let id: String = map_ref.get_with_txn(txn, VIEW_ID)?;
  let name: String = map_ref.get_with_txn(txn, VIEW_NAME)?;
  let database_id: String = map_ref
    .get_with_txn(txn, VIEW_DATABASE_ID)
    .unwrap_or_default();
  let layout = map_ref
    .get_with_txn::<_, i64>(txn, DATABASE_VIEW_LAYOUT)
    .map(DatabaseLayout::from)?;

  let layout_settings = map_ref
    .get_with_txn::<_, MapRef>(txn, VIEW_LAYOUT_SETTINGS)
    .map(|map_ref| LayoutSettings::from_map_ref(txn, map_ref))
    .unwrap_or_default();

  let filters = map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_FILTERS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default();

  let group_settings = map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_GROUPS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default();

  let sorts = map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_SORTS)
    .map(|array_ref| array_of_maps(array_ref, txn))
    .unwrap_or_default();

  let row_orders = map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_ROW_ORDERS)
    .map(|array_ref| RowOrderArray::new(array_ref).get_objects_with_txn(txn))
    .unwrap_or_default();

  let field_orders = map_ref
    .get_with_txn::<_, ArrayRef>(txn, DATABASE_VIEW_FIELD_ORDERS)
    .map(|array_ref| FieldOrderArray::new(array_ref).get_objects_with_txn(txn))
    .unwrap_or_default();

  let created_at: i64 = map_ref
    .get_with_txn(txn, VIEW_CREATE_AT)
    .unwrap_or_default();

  let modified_at: i64 = map_ref
    .get_with_txn(txn, VIEW_MODIFY_AT)
    .unwrap_or_default();

  let field_settings = map_ref
    .get_with_txn::<_, MapRef>(txn, DATABASE_VIEW_FIELD_SETTINGS)
    .map(|map_ref| FieldSettingsByFieldIdMap::from((txn, &map_ref)))
    .unwrap_or_default();

  let is_inline: bool = map_ref.get_with_txn(txn, IS_INLINE).unwrap_or(false);

  Some(DatabaseView {
    id,
    database_id,
    name,
    layout,
    layout_settings,
    filters,
    group_settings,
    sorts,
    row_orders,
    field_orders,
    field_settings,
    created_at,
    modified_at,
    is_inline,
  })
}

pub trait OrderIdentifiable {
  fn identify_id(&self) -> String;
}

/// The [OrderArray] trait provides a set of methods to manipulate an array of [OrderIdentifiable] objects.
pub trait OrderArray {
  type Object: OrderIdentifiable + Into<Any>;

  /// Returns the array reference.
  fn array_ref(&self) -> &ArrayRef;

  /// Create a new [Self::Object] from given value
  fn object_from_value<T: ReadTxn>(&self, value: YrsValue, txn: &T) -> Option<Self::Object>;

  /// Extends the other objects to the end of the array.
  fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Self::Object>) {
    let array_ref = self.array_ref();
    for order in others {
      array_ref.push_back(txn, order);
    }
  }

  /// Pushes the given object to the front of the array.
  fn push_front_with_txn(&self, txn: &mut TransactionMut, object: Self::Object) {
    self.array_ref().push_front(txn, object);
  }

  /// Pushes the given object to the end of the array.
  fn push_back_with_txn(&self, txn: &mut TransactionMut, object: Self::Object) {
    self.array_ref().push_back(txn, object);
  }

  /// Insert the given object to the array before the given previous object.
  fn insert_before_with_txn(
    &self,
    txn: &mut TransactionMut,
    object: Self::Object,
    next_object_id: &str,
  ) {
    match self.get_position_with_txn(txn, next_object_id) {
      Some(pos) => self.array_ref().insert(txn, pos, object),
      None => {
        tracing::warn!(
          "\"{}\" isn't found in the order array, appending to the end instead",
          next_object_id
        );
        self.array_ref().push_back(txn, object)
      },
    };
  }

  /// Insert the given object to the array after the given previous object.
  fn insert_after_with_txn(
    &self,
    txn: &mut TransactionMut,
    object: Self::Object,
    prev_object_id: &str,
  ) {
    match self.get_position_with_txn(txn, prev_object_id) {
      Some(pos) => {
        let next: u32 = pos + 1;
        self.array_ref().insert(txn, next, object)
      },
      None => {
        tracing::warn!(
          "\"{}\" isn't found in the order array, appending to the end instead",
          prev_object_id
        );
        self.array_ref().push_back(txn, object)
      },
    };
  }

  /// Returns a list of Objects with a transaction.
  fn get_objects_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Self::Object> {
    self
      .array_ref()
      .iter(txn)
      .flat_map(|v| self.object_from_value(v, txn))
      .collect::<Vec<Self::Object>>()
  }

  fn get_object_at_index<T: ReadTxn>(&self, txn: &T, index: u32) -> Option<Self::Object> {
    self
      .array_ref()
      .get(txn, index)
      .and_then(|value| self.object_from_value(value, txn))
  }

  fn replace_with_txn(&self, txn: &mut TransactionMut, object: Self::Object) {
    if let Some(pos) =
      self
        .array_ref()
        .iter(txn)
        .position(|value| match self.object_from_value(value, txn) {
          None => false,
          Some(order) => order.identify_id() == object.identify_id(),
        })
    {
      self.array_ref().remove(txn, pos as u32);
      self.array_ref().insert(txn, pos as u32, object);
    } else {
      tracing::warn!("Can't replace the object. The object is not found")
    }
  }

  // Remove the object with the given id from the array.
  fn remove_with_txn(&self, txn: &mut TransactionMut, id: &str) -> Option<()> {
    let pos = self.get_position_with_txn(txn, id)?;
    self.array_ref().remove(txn, pos);
    None
  }

  /// Move the object with the given id to the given position.
  /// If the object is not found, nothing will happen.
  /// If the position is out of range, nothing will happen.
  fn move_to(&self, txn: &mut TransactionMut, from_id: &str, to_id: &str) -> Option<()> {
    let from = self.get_position_with_txn(txn, from_id)?;
    let to = self.get_position_with_txn(txn, to_id)?;
    let to = if from < to { to + 1 } else { to };
    trace!("Move object from {} to {}", from, to);
    self.array_ref().move_to(txn, from, to);
    None
  }

  /// Returns the position of the object with the given id.
  fn get_position_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<u32> {
    self
      .array_ref()
      .iter(txn)
      .position(|value| {
        let object = self.object_from_value(value, txn);
        match object {
          None => false,
          Some(order) => order.identify_id() == id,
        }
      })
      .map(|pos| pos as u32)
  }
}
