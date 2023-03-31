use crate::views::layout::{Layout, LayoutSettings};
use crate::views::{
  FieldOrder, FieldOrderArray, Filter, FilterArray, Group, GroupArray, RowOrder, RowOrderArray,
  Sort, SortArray,
};
use crate::{impl_any_update, impl_order_update, impl_str_update};
use collab::preclude::map::MapPrelim;
use collab::preclude::{
  lib0Any, Array, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct View {
  pub id: String,
  pub database_id: String,
  pub name: String,
  pub layout: Layout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<Filter>,
  pub groups: Vec<Group>,
  pub sorts: Vec<Sort>,
  pub row_orders: Vec<RowOrder>,
  pub field_orders: Vec<FieldOrder>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateViewParams {
  pub id: String,
  pub database_id: String,
  pub name: String,
  pub layout: Layout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<Filter>,
  pub groups: Vec<Group>,
  pub sorts: Vec<Sort>,
}

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_DATABASE_ID: &str = "database_id";
const VIEW_LAYOUT: &str = "layout";
const VIEW_LAYOUT_SETTINGS: &str = "layout_settings";
const VIEW_FILTERS: &str = "filters";
const VIEW_GROUPS: &str = "groups";
const VIEW_SORTS: &str = "sorts";
const ROW_ORDERS: &str = "row_orders";
const FIELD_ORDERS: &str = "field_orders";

pub struct ViewBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_with_txn(txn, VIEW_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(ViewUpdate),
  {
    let map_ref_ext = MapRefExtension(&self.map_ref);
    let update = ViewUpdate::new(self.id, self.txn, map_ref_ext);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct ViewUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: MapRefExtension<'c>,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> ViewUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefExtension<'c>) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(
    set_database_id,
    set_database_id_if_not_none,
    VIEW_DATABASE_ID
  );

  impl_str_update!(set_name, set_name_if_not_none, VIEW_NAME);

  impl_any_update!(
    set_layout_type,
    set_layout_type_if_not_none,
    VIEW_LAYOUT,
    Layout
  );

  pub fn set_layout_settings(self, layout_settings: LayoutSettings) -> Self {
    let map_ref = self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, VIEW_LAYOUT_SETTINGS);
    layout_settings.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn set_filter(self, filters: Vec<Filter>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_FILTERS);
    let filter_array = FilterArray::new(array_ref);
    filter_array.extends_with_txn(self.txn, filters);
    self
  }

  pub fn set_groups(self, groups: Vec<Group>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_GROUPS);
    let filter_array = GroupArray::new(array_ref);
    filter_array.extends_with_txn(self.txn, groups);
    self
  }

  pub fn set_sorts(self, sorts: Vec<Sort>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_SORTS);
    let sort_array = SortArray::new(array_ref);
    sort_array.extends_with_txn(self.txn, sorts);
    self
  }

  impl_order_update!(
    set_row_orders,
    add_row_order,
    remove_row_order,
    ROW_ORDERS,
    RowOrder,
    RowOrderArray
  );

  impl_order_update!(
    set_field_orders,
    add_field_order,
    remove_field_order,
    FIELD_ORDERS,
    FieldOrder,
    FieldOrderArray
  );

  pub fn done(self) -> Option<View> {
    view_from_map_ref(self.map_ref.into_inner(), self.txn)
  }
}

pub fn view_id_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<String> {
  MapRefExtension(map_ref).get_str_with_txn(txn, VIEW_ID)
}

pub fn view_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<View> {
  let map_ref = value.to_ymap()?;
  view_from_map_ref(&map_ref, txn)
}

pub fn view_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<View> {
  let map_ref = MapRefExtension(map_ref);
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
    .map(|array_ref| FilterArray::new(array_ref).get_filters_with_txn(txn))
    .unwrap_or_default();

  let groups = map_ref
    .get_array_ref_with_txn(txn, VIEW_GROUPS)
    .map(|array_ref| GroupArray::new(array_ref).get_groups_with_txn(txn))
    .unwrap_or_default();

  let sorts = map_ref
    .get_array_ref_with_txn(txn, VIEW_SORTS)
    .map(|array_ref| SortArray::new(array_ref).get_sorts_with_txn(txn))
    .unwrap_or_default();

  let row_orders = map_ref
    .get_array_ref_with_txn(txn, ROW_ORDERS)
    .map(|array_ref| RowOrderArray::new(array_ref).get_row_orders_with_txn(txn))
    .unwrap_or_default();

  let field_orders = map_ref
    .get_array_ref_with_txn(txn, FIELD_ORDERS)
    .map(|array_ref| FieldOrderArray::new(array_ref).get_field_orders_with_txn(txn))
    .unwrap_or_default();

  Some(View {
    id,
    database_id,
    name,
    layout,
    layout_settings,
    filters,
    groups,
    sorts,
    row_orders,
    field_orders,
  })
}
