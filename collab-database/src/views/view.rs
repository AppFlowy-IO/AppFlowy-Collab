use crate::database::gen_database_view_id;
use crate::fields::Field;
use crate::rows::Row;
use crate::views::layout::{Layout, LayoutSettings};
use crate::views::{
  FieldOrder, FieldOrderArray, Filter, FilterArray, GroupSetting, GroupSettingArray, RowOrder,
  RowOrderArray, Sort, SortArray,
};
use crate::{impl_any_update, impl_i64_update, impl_order_update, impl_str_update};
use collab::preclude::map::MapPrelim;
use collab::preclude::{
  lib0Any, Array, ArrayRef, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
  YrsValue,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabaseView {
  pub id: String,
  pub database_id: String,
  pub name: String,
  pub layout: Layout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<Filter>,
  pub groups: Vec<GroupSetting>,
  pub sorts: Vec<Sort>,
  pub row_orders: Vec<RowOrder>,
  pub field_orders: Vec<FieldOrder>,
  pub created_at: i64,
  pub modified_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateViewParams {
  pub view_id: String,
  pub name: String,
  pub layout: Layout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<Filter>,
  pub groups: Vec<GroupSetting>,
  pub sorts: Vec<Sort>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateDatabaseParams {
  pub view_id: String,
  pub name: String,
  pub layout: Layout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<Filter>,
  pub groups: Vec<GroupSetting>,
  pub sorts: Vec<Sort>,
  pub rows: Vec<Row>,
  pub fields: Vec<Field>,
}

impl CreateDatabaseParams {
  pub fn from_view(view: DatabaseView, rows: Vec<Row>, fields: Vec<Field>) -> Self {
    let mut params: Self = view.into();
    params.rows = rows;
    params.fields = fields;
    params
  }
  pub fn split(self) -> (Vec<Row>, Vec<Field>, CreateViewParams) {
    (
      self.rows,
      self.fields,
      CreateViewParams {
        view_id: self.view_id,
        name: self.name,
        layout: self.layout,
        layout_settings: self.layout_settings,
        filters: self.filters,
        groups: self.groups,
        sorts: self.sorts,
      },
    )
  }
}

impl From<DatabaseView> for CreateDatabaseParams {
  fn from(view: DatabaseView) -> Self {
    Self {
      view_id: gen_database_view_id(),
      name: view.name,
      layout: view.layout,
      layout_settings: view.layout_settings,
      filters: view.filters,
      groups: view.groups,
      sorts: view.sorts,
      rows: vec![],
      fields: vec![],
    }
  }
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
const VIEW_CREATE_AT: &str = "created_at";
const VIEW_MODIFY_AT: &str = "modified_at";

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
    let update = ViewUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct ViewUpdate<'a, 'b> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewUpdate<'a, 'b> {
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
    Layout
  );

  impl_order_update!(
    set_row_orders,
    add_row_order,
    remove_row_order,
    move_row_order,
    insert_row_order,
    ROW_ORDERS,
    RowOrder,
    RowOrderArray
  );

  impl_order_update!(
    set_field_orders,
    add_field_order,
    remove_field_order,
    move_field_order,
    insert_field_order,
    FIELD_ORDERS,
    FieldOrder,
    FieldOrderArray
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

  pub fn set_groups(self, groups: Vec<GroupSetting>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, VIEW_GROUPS);
    let filter_array = GroupSettingArray::new(array_ref);
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

  pub fn done(self) -> Option<DatabaseView> {
    view_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn view_id_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<String> {
  map_ref.get_str_with_txn(txn, VIEW_ID)
}

pub fn view_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<DatabaseView> {
  let map_ref = value.to_ymap()?;
  view_from_map_ref(&map_ref, txn)
}

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
    .map(|array_ref| FilterArray::new(array_ref).get_filters_with_txn(txn))
    .unwrap_or_default();

  let groups = map_ref
    .get_array_ref_with_txn(txn, VIEW_GROUPS)
    .map(|array_ref| GroupSettingArray::new(array_ref).get_group_setting_with_txn(txn))
    .unwrap_or_default();

  let sorts = map_ref
    .get_array_ref_with_txn(txn, VIEW_SORTS)
    .map(|array_ref| SortArray::new(array_ref).get_sorts_with_txn(txn))
    .unwrap_or_default();

  let row_orders = map_ref
    .get_array_ref_with_txn(txn, ROW_ORDERS)
    .map(|array_ref| RowOrderArray::new(array_ref).get_orders_with_txn(txn))
    .unwrap_or_default();

  let field_orders = map_ref
    .get_array_ref_with_txn(txn, FIELD_ORDERS)
    .map(|array_ref| FieldOrderArray::new(array_ref).get_orders_with_txn(txn))
    .unwrap_or_default();

  let created_at = map_ref
    .get_i64_with_txn(txn, VIEW_CREATE_AT)
    .unwrap_or_default();

  let modified_at = map_ref
    .get_i64_with_txn(txn, VIEW_MODIFY_AT)
    .unwrap_or_default();

  Some(DatabaseView {
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
    created_at,
    modified_at,
  })
}

pub trait OrderIdentifiable {
  fn identify_id(&self) -> &str;
}

pub trait OrderArray {
  type Object: OrderIdentifiable + Into<lib0Any>;

  fn array_ref(&self) -> &ArrayRef;

  fn object_from_value_with_txn<T: ReadTxn>(
    &self,
    value: YrsValue,
    txn: &T,
  ) -> Option<Self::Object>;

  fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Self::Object>) {
    let array_ref = self.array_ref();
    for order in others {
      array_ref.push_back(txn, order);
    }
  }

  fn push_with_txn(&self, txn: &mut TransactionMut, object: Self::Object) {
    self.array_ref().push_back(txn, object);
  }

  fn insert_with_txn(&self, txn: &mut TransactionMut, object: Self::Object, prev_object_id: &str) {
    if prev_object_id.is_empty() {
      self.array_ref().push_front(txn, object);
      return;
    }

    match self.get_row_position_with_txn(txn, prev_object_id) {
      None => {
        self.array_ref().push_back(txn, object);
      },
      Some(pos) => {
        let next: u32 = pos as u32 + 1;
        self.array_ref().insert(txn, next, object);
      },
    }
  }

  fn get_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Self::Object> {
    self
      .array_ref()
      .iter(txn)
      .flat_map(|v| self.object_from_value_with_txn(v, txn))
      .collect::<Vec<Self::Object>>()
  }

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

  fn move_to(&self, txn: &mut TransactionMut, from: u32, to: u32) {
    let array_ref = self.array_ref();
    if let Some(YrsValue::Any(value)) = array_ref.get(txn, from) {
      array_ref.remove(txn, from);
      array_ref.insert(txn, to, value);
    }
  }

  fn get_row_position_with_txn<T: ReadTxn>(&self, txn: &T, row_id: &str) -> Option<u32> {
    self
      .array_ref()
      .iter(txn)
      .position(|value| match self.object_from_value_with_txn(value, txn) {
        None => false,
        Some(order) => order.identify_id() == row_id,
      })
      .map(|pos| pos as u32)
  }
}
