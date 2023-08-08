use collab::core::any_array::ArrayMapUpdate;
use collab::core::any_map::AnyMapUpdate;
use collab::preclude::map::MapPrelim;
use collab::preclude::{
  lib0Any, Array, ArrayRef, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
  YrsValue,
};
use serde::{Deserialize, Serialize};

use crate::error::DatabaseError;
use crate::fields::Field;
use crate::rows::CreateRowParams;
use crate::views::layout::{DatabaseLayout, LayoutSettings};
use crate::views::{
  FieldOrder, FieldOrderArray, FieldSetting, FieldSettingsMap, FilterArray, FilterMap, GroupSettingArray,
  GroupSettingMap, LayoutSetting, RowOrder, RowOrderArray, SortArray, SortMap,
};
use crate::{impl_any_update, impl_i64_update, impl_order_update, impl_str_update};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DatabaseView {
  pub id: String,
  pub database_id: String,
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<FilterMap>,
  pub group_settings: Vec<GroupSettingMap>,
  pub sorts: Vec<SortMap>,
  pub row_orders: Vec<RowOrder>,
  pub field_orders: Vec<FieldOrder>,
  pub field_settings: FieldSettingsMap,
  pub created_at: i64,
  pub modified_at: i64,
}

pub struct ViewDescription {
  pub id: String,
  pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateViewParams {
  pub database_id: String,
  pub view_id: String,
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<FilterMap>,
  pub groups: Vec<GroupSettingMap>,
  pub sorts: Vec<SortMap>,
  pub field_settings: FieldSettingsMap,
  /// When creating a view for a database, it might need to create a new field for the view.
  /// For example, if the view is calendar view, it must have a date field.
  pub deps_fields: Vec<Field>,
  pub deps_field_setting: Option<FieldSetting>,
}

impl CreateViewParams {
  pub fn take_deps_fields(&mut self) -> Vec<Field> {
    std::mem::take(&mut self.deps_fields)
  }

  pub fn take_deps_field_setting(&mut self) -> Option<FieldSetting> {
    std::mem::take(&mut self.deps_field_setting)
  }
}

impl CreateViewParams {
  pub fn new(database_id: String, view_id: String, name: String, layout: DatabaseLayout) -> Self {
    Self {
      database_id,
      view_id,
      name,
      layout,
      ..Default::default()
    }
  }

  pub fn with_layout_setting(mut self, layout_setting: LayoutSetting) -> Self {
    self.layout_settings.insert(self.layout, layout_setting);
    self
  }

  pub fn with_filters(mut self, filters: Vec<FilterMap>) -> Self {
    self.filters = filters;
    self
  }

  pub fn with_groups(mut self, groups: Vec<GroupSettingMap>) -> Self {
    self.groups = groups;
    self
  }

  pub fn with_deps_fields(mut self, fields: Vec<Field>) -> Self {
    self.deps_fields = fields;
    self
  }
}

pub(crate) struct CreateViewParamsValidator;

impl CreateViewParamsValidator {
  pub(crate) fn validate(params: CreateViewParams) -> Result<CreateViewParams, DatabaseError> {
    if params.database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    if params.view_id.is_empty() {
      return Err(DatabaseError::InvalidViewID("view_id is empty"));
    }

    Ok(params)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateDatabaseParams {
  pub database_id: String,
  pub view_id: String,
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<FilterMap>,
  pub groups: Vec<GroupSettingMap>,
  pub sorts: Vec<SortMap>,
  pub field_settings: FieldSettingsMap,
  pub created_rows: Vec<CreateRowParams>,
  pub fields: Vec<Field>,
}

impl CreateDatabaseParams {
  pub fn from_view(view: DatabaseView, fields: Vec<Field>, rows: Vec<CreateRowParams>) -> Self {
    let mut params: Self = view.into();
    params.fields = fields;
    params.created_rows = rows;
    params
  }

  pub fn split(self) -> (Vec<CreateRowParams>, Vec<Field>, CreateViewParams) {
    (
      self.created_rows,
      self.fields,
      CreateViewParams {
        database_id: self.database_id,
        view_id: self.view_id,
        name: self.name,
        layout: self.layout,
        layout_settings: self.layout_settings,
        filters: self.filters,
        groups: self.groups,
        sorts: self.sorts,
        field_settings: self.field_settings,
        deps_fields: vec![],
        deps_field_setting: None,
      },
    )
  }
}

impl From<DatabaseView> for CreateDatabaseParams {
  fn from(view: DatabaseView) -> Self {
    Self {
      database_id: view.database_id,
      view_id: view.id,
      name: view.name,
      layout: view.layout,
      layout_settings: view.layout_settings,
      filters: view.filters,
      groups: view.group_settings,
      sorts: view.sorts,
      field_settings: view.field_settings,
      created_rows: vec![],
      fields: vec![],
    }
  }
}

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_DATABASE_ID: &str = "database_id";
pub const VIEW_LAYOUT: &str = "layout";
const VIEW_LAYOUT_SETTINGS: &str = "layout_settings";
const VIEW_FILTERS: &str = "filters";
const VIEW_GROUPS: &str = "groups";
const VIEW_SORTS: &str = "sorts";
const VIEW_FIELD_SETTINGS: &str = "field_settings";
pub const ROW_ORDERS: &str = "row_orders";
pub const FIELD_ORDERS: &str = "field_orders";
const VIEW_CREATE_AT: &str = "created_at";
const VIEW_MODIFY_AT: &str = "modified_at";

pub struct ViewBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_str_with_txn(txn, VIEW_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(DatabaseViewUpdate),
  {
    let update = DatabaseViewUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct DatabaseViewUpdate<'a, 'b> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> DatabaseViewUpdate<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { id, map_ref, txn }
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
    VIEW_LAYOUT,
    DatabaseLayout
  );

  impl_order_update!(
    set_row_orders,
    push_row_order,
    remove_row_order,
    move_row_order,
    insert_row_order,
    ROW_ORDERS,
    RowOrder,
    RowOrderArray
  );

  impl_order_update!(
    set_field_orders,
    push_field_order,
    remove_field_order,
    move_field_order,
    insert_field_order,
    FIELD_ORDERS,
    FieldOrder,
    FieldOrderArray
  );

  /// Set layout settings of the current view
  pub fn set_layout_settings(self, layout_settings: LayoutSettings) -> Self {
    let map_ref = self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, VIEW_LAYOUT_SETTINGS);
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
    let layout_settings = self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, VIEW_LAYOUT_SETTINGS);

    let layout_setting_map =
      layout_settings.get_or_insert_map_with_txn(self.txn, layout_ty.as_ref());
    layout_setting.fill_map_ref(self.txn, &layout_setting_map);
    self
  }

  /// Remove layout setting for the given [DatabaseLayout]
  pub fn remove_layout_setting(self, layout_ty: &DatabaseLayout) -> Self {
    let layout_settings = self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, VIEW_LAYOUT_SETTINGS);

    layout_settings.remove(self.txn, layout_ty.as_ref());
    self
  }

  /// Set filters of the current view
  pub fn set_filters(mut self, filters: Vec<FilterMap>) -> Self {
    let array_ref = self.get_filter_array();
    let filter_array = FilterArray::from_any_maps(filters);
    filter_array.set_array_ref(self.txn, array_ref);
    self
  }

  /// Update filters
  /// The given function, [ArrayMapUpdate], which can be used to update the filters
  pub fn update_filters<F>(mut self, f: F) -> Self
  where
    F: FnOnce(ArrayMapUpdate),
  {
    let array_ref = self.get_filter_array();
    let update = ArrayMapUpdate::new(self.txn, array_ref);
    f(update);
    self
  }

  /// Set groups of the current view
  pub fn set_groups(mut self, group_settings: Vec<GroupSettingMap>) -> Self {
    let array_ref = self.get_group_array();
    let group_settings = GroupSettingArray::from_any_maps(group_settings);
    group_settings.set_array_ref(self.txn, array_ref);
    self
  }

  /// Update groups
  /// The given function, [ArrayMapUpdate], which can be used to update the groups
  pub fn update_groups<F>(mut self, f: F) -> Self
  where
    F: FnOnce(ArrayMapUpdate),
  {
    let array_ref = self.get_group_array();
    let update = ArrayMapUpdate::new(self.txn, array_ref);
    f(update);
    self
  }

  /// Set sorts of the current view
  pub fn set_sorts(mut self, sorts: Vec<SortMap>) -> Self {
    let array_ref = self.get_sort_array();
    let sort_array = SortArray::from_any_maps(sorts);
    sort_array.set_array_ref(self.txn, array_ref);
    self
  }

  /// Update sorts
  /// The given function, [ArrayMapUpdate], which can be used to update the sorts
  pub fn update_sorts<F>(mut self, f: F) -> Self
  where
    F: FnOnce(ArrayMapUpdate),
  {
    let array_ref = self.get_sort_array();
    let update = ArrayMapUpdate::new(self.txn, array_ref);
    f(update);
    self
  }

  /// Set the field settings of the current view
  pub fn set_field_settings(mut self, field_settings: FieldSettingsMap) -> Self {
    let map_ref = self.get_field_settings();
    field_settings.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn update_field_settings<F>(mut self, field_ids: Vec<String>, f: F) -> Self
  where
    F: Fn(&str, AnyMapUpdate),
  {
    let map_ref = self.get_field_settings();
    field_ids.iter().for_each(|field_id| {
      let update = AnyMapUpdate::new(self.txn, &map_ref);
      f(field_id.as_str(), update);
    });
    self
  }

  pub fn update_field_settings_one<F>(mut self, field_id: &str, f: F) -> Self
  where
    F: FnOnce(AnyMapUpdate),
  {
    let map_ref = self.get_field_settings();
    let update = AnyMapUpdate::new(self.txn, &map_ref);
    f(update);
    self
  }

  // TODO: set

  /// Get the sort array for the curent view, used when setting or updating
  /// sort array
  fn get_sort_array(&mut self) -> ArrayRef {
    self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_SORTS)
  }

  /// Get the group array for the curent view, used when setting or updating
  /// group array
  fn get_group_array(&mut self) -> ArrayRef {
    self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_GROUPS)
  }

  /// Get the filter array for the curent view, used when setting or updating
  /// filter array
  fn get_filter_array(&mut self) -> ArrayRef {
    self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_FILTERS)
  }

  /// Get the field settings for the curent view, used when setting or updating
  /// field settings
  fn get_field_settings(&mut self) -> MapRef {
    self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, VIEW_FIELD_SETTINGS)
  }

  pub fn done(self) -> Option<DatabaseView> {
    view_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn view_id_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<String> {
  map_ref.get_str_with_txn(txn, VIEW_ID)
}

/// Return a [ViewDescription] from a map ref
/// A [ViewDescription] is a subset of a [DatabaseView]
pub fn view_description_from_value<T: ReadTxn>(
  value: YrsValue,
  txn: &T,
) -> Option<ViewDescription> {
  let map_ref = value.to_ymap()?;
  let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
  let name = map_ref.get_str_with_txn(txn, VIEW_NAME).unwrap_or_default();
  Some(ViewDescription { id, name })
}

/// Return a [DatabaseView] from a map ref
pub fn view_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<DatabaseView> {
  let map_ref = value.to_ymap()?;
  view_from_map_ref(&map_ref, txn)
}

/// Return a list of [GroupSettingMap] from a map ref
pub fn group_setting_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<GroupSettingMap> {
  map_ref
    .get_array_ref_with_txn(txn, VIEW_GROUPS)
    .map(|array_ref| GroupSettingArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default()
}

/// Return a new list of [SortMap]s from a map ref
pub fn sorts_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<SortMap> {
  map_ref
    .get_array_ref_with_txn(txn, VIEW_SORTS)
    .map(|array_ref| SortArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default()
}

/// Return a new list of [FilterMap]s from a map ref
pub fn filters_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Vec<FilterMap> {
  map_ref
    .get_array_ref_with_txn(txn, VIEW_FILTERS)
    .map(|array_ref| FilterArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default()
}

/// Creates a new layout settings from a map ref
pub fn layout_setting_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> LayoutSettings {
  map_ref
    .get_map_with_txn(txn, VIEW_LAYOUT_SETTINGS)
    .map(|map_ref| LayoutSettings::from_map_ref(txn, map_ref))
    .unwrap_or_default()
}

/// Creates a new field settings from a map ref
pub fn field_settings_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> FieldSettingsMap {
  map_ref
    .get_map_with_txn(txn, VIEW_FIELD_SETTINGS)
    .map(|map_ref| FieldSettingsMap::from_map_ref(txn, &map_ref))
    .unwrap_or_default()
}

/// Creates a new view from a map ref
pub fn view_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<DatabaseView> {
  let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
  let name = map_ref.get_str_with_txn(txn, VIEW_NAME)?;
  let database_id = map_ref
    .get_str_with_txn(txn, VIEW_DATABASE_ID)
    .unwrap_or_default();
  let layout = map_ref
    .get_i64_with_txn(txn, VIEW_LAYOUT)
    .map(|value| value.try_into().ok())??;

  let layout_settings = map_ref
    .get_map_with_txn(txn, VIEW_LAYOUT_SETTINGS)
    .map(|map_ref| LayoutSettings::from_map_ref(txn, map_ref))
    .unwrap_or_default();

  let filters = map_ref
    .get_array_ref_with_txn(txn, VIEW_FILTERS)
    .map(|array_ref| FilterArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default();

  let group_settings = map_ref
    .get_array_ref_with_txn(txn, VIEW_GROUPS)
    .map(|array_ref| GroupSettingArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default();

  let sorts = map_ref
    .get_array_ref_with_txn(txn, VIEW_SORTS)
    .map(|array_ref| SortArray::from_array_ref(txn, &array_ref).0)
    .unwrap_or_default();

  let row_orders = map_ref
    .get_array_ref_with_txn(txn, ROW_ORDERS)
    .map(|array_ref| RowOrderArray::new(array_ref).get_objects_with_txn(txn))
    .unwrap_or_default();

  let field_orders = map_ref
    .get_array_ref_with_txn(txn, FIELD_ORDERS)
    .map(|array_ref| FieldOrderArray::new(array_ref).get_objects_with_txn(txn))
    .unwrap_or_default();

  let created_at = map_ref
    .get_i64_with_txn(txn, VIEW_CREATE_AT)
    .unwrap_or_default();

  let modified_at = map_ref
    .get_i64_with_txn(txn, VIEW_MODIFY_AT)
    .unwrap_or_default();

  let field_settings = map_ref
    .get_map_with_txn(txn, VIEW_FIELD_SETTINGS)
    .map(|map_ref| FieldSettingsMap::from_map_ref(txn, &map_ref))
    .unwrap_or_default();

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
  })
}

pub trait OrderIdentifiable {
  fn identify_id(&self) -> String;
}

/// The [OrderArray] trait provides a set of methods to manipulate an array of [OrderIdentifiable] objects.
pub trait OrderArray {
  type Object: OrderIdentifiable + Into<lib0Any>;

  /// Returns the array reference.
  fn array_ref(&self) -> &ArrayRef;

  /// Create a new [Self::Object] from given value
  fn object_from_value_with_txn<T: ReadTxn>(
    &self,
    value: YrsValue,
    txn: &T,
  ) -> Option<Self::Object>;

  /// Extends the other objects to the end of the array.
  fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Self::Object>) {
    let array_ref = self.array_ref();
    for order in others {
      array_ref.push_back(txn, order);
    }
  }

  /// Pushes the given object to the end of the array.
  fn push_with_txn(&self, txn: &mut TransactionMut, object: Self::Object) {
    self.array_ref().push_back(txn, object);
  }

  /// Insert the given object to the array after the given previous object.
  /// If the previous object is not found, the object will be inserted to the end of the array.
  /// If the previous object is None, the object will be inserted to the beginning of the array.
  fn insert_with_txn(
    &self,
    txn: &mut TransactionMut,
    object: Self::Object,
    prev_object_id: Option<&String>,
  ) {
    if let Some(prev_object_id) = prev_object_id {
      match self.get_position_with_txn(txn, &prev_object_id.to_owned()) {
        None => {
          self.array_ref().push_back(txn, object);
        },
        Some(pos) => {
          let next: u32 = pos + 1;
          self.array_ref().insert(txn, next, object);
        },
      }
    } else {
      self.array_ref().push_front(txn, object);
    }
  }

  /// Returns a list of Objects with a transaction.
  fn get_objects_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Self::Object> {
    self
      .array_ref()
      .iter(txn)
      .flat_map(|v| self.object_from_value_with_txn(v, txn))
      .collect::<Vec<Self::Object>>()
  }

  // Remove the object with the given id from the array.
  fn remove_with_txn(&self, txn: &mut TransactionMut, id: &str) -> Option<()> {
    let pos = self.array_ref().iter(txn).position(|value| {
      match self.object_from_value_with_txn(value, txn) {
        None => false,
        Some(order) => order.identify_id() == id,
      }
    })?;
    self.array_ref().remove(txn, pos as u32);
    None
  }

  /// Move the object with the given id to the given position.
  /// If the object is not found, nothing will happen.
  /// If the position is out of range, nothing will happen.
  fn move_to(&self, txn: &mut TransactionMut, from: u32, to: u32) {
    let array_ref = self.array_ref();
    if let Some(YrsValue::Any(value)) = array_ref.get(txn, from) {
      if to <= array_ref.len(txn) {
        array_ref.remove(txn, from);
        array_ref.insert(txn, to, value);
      }
    }
  }

  /// Returns the position of the object with the given id.
  fn get_position_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<u32> {
    self
      .array_ref()
      .iter(txn)
      .position(|value| {
        let object = self.object_from_value_with_txn(value, txn);
        match object {
          None => false,
          Some(order) => order.identify_id() == id,
        }
      })
      .map(|pos| pos as u32)
  }
}
