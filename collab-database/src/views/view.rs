use crate::views::layout::{Layout, LayoutSettings};
use crate::views::{Filter, FilterArray, Group, GroupArray, Sort, SortArray};
use crate::{impl_any_update, impl_str_update};
use collab::preclude::map::MapPrelim;
use collab::preclude::{lib0Any, MapRef, MapRefTool, MapRefWrapper, ReadTxn, TransactionMut};
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
}

const VIEW_ID: &str = "id";
const VIEW_DATABASE_ID: &str = "database_id";
const VIEW_LAYOUT: &str = "layout";
const VIEW_LAYOUT_SETTINGS: &str = "layout_settings";
const VIEW_FILTERS: &str = "filters";
const VIEW_GROUPS: &str = "groups";
const VIEW_SORTS: &str = "sorts";

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

pub struct ViewUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> ViewUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(
    set_database_id,
    set_database_id_if_not_none,
    VIEW_DATABASE_ID
  );
  impl_any_update!(
    set_field_type,
    set_field_type_if_not_none,
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

  pub fn done(self) -> Option<View> {
    view_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn view_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<View> {
  todo!()
  // let map_ref = MapRefTool(map_ref);
  //
  // let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
  //
  // let name = map_ref
  //   .get_str_with_txn(txn, VIEW_DATABASE_ID)
  //   .unwrap_or_default();
  //
  // let visibility = map_ref.get_bool_with_txn(txn, VIEW_FILTERS).unwrap_or(true);
  //
  // let width = map_ref.get_i64_with_txn(txn, VIEW_GROUPS).unwrap_or(120);
  //
  // let type_options = map_ref
  //   .get_map_ref_with_txn(txn, VIEW_LAYOUT_SETTINGS)
  //   .map(|map_ref| TypeOptions::from_map_ref(txn, map_ref))
  //   .unwrap_or_default();
  //
  // let field_type = map_ref
  //   .get_i64_with_txn(txn, VIEW_LAYOUT)
  //   .map(|value| value.try_into().ok())??;
  //
  // let is_primary = map_ref.get_bool_with_txn(txn, VIEW_SORTS).unwrap_or(false);
  //
  // Some(Field {
  //   id,
  //   name,
  //   field_type,
  //   visibility,
  //   width,
  //   type_options,
  //   is_primary,
  // })
}
