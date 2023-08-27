use collab::preclude::{MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};

use crate::{impl_bool_update, impl_i64_update, impl_str_update};
use crate::fields::{TypeOptionData, TypeOptions, TypeOptionsUpdate};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Field {
  pub id: String,
  pub name: String,
  pub field_type: i64,
  pub visibility: bool,
  pub width: i64,
  pub type_options: TypeOptions,
  #[serde(default = "DEFAULT_IS_PRIMARY_VALUE")]
  pub is_primary: bool,
}

impl Field {
  pub fn new(id: String, name: String, field_type: i64, is_primary: bool) -> Self {
    Self {
      id,
      name,
      field_type,
      visibility: true,
      width: 120,
      type_options: Default::default(),
      is_primary,
    }
  }

  pub fn with_type_option_data(
    mut self,
    type_id: impl ToString,
    type_options: TypeOptionData,
  ) -> Self {
    self.type_options.insert(type_id.to_string(), type_options);
    self
  }

  pub fn get_type_option<T: From<TypeOptionData>>(&self, type_id: impl ToString) -> Option<T> {
    let type_option_data = self.type_options.get(&type_id.to_string())?.clone();
    Some(T::from(type_option_data))
  }

  pub fn get_any_type_option(&self, type_id: impl ToString) -> Option<TypeOptionData> {
    self.type_options.get(&type_id.to_string()).cloned()
  }
}

const DEFAULT_IS_PRIMARY_VALUE: fn() -> bool = || false;

pub struct FieldBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> FieldBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_with_txn(txn, FIELD_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(FieldUpdate),
  {
    let update = FieldUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct FieldUpdate<'a, 'b, 'c> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'c MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> FieldUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRef) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_name, set_name_if_not_none, FIELD_NAME);
  impl_bool_update!(set_visibility, set_visibility_if_not_none, FIELD_VISIBILITY);
  impl_bool_update!(set_primary, set_primary_if_not_none, FIELD_PRIMARY);
  impl_i64_update!(set_width, set_width_at_if_not_none, FIELD_WIDTH);
  impl_i64_update!(set_field_type, set_field_type_if_not_none, FIELD_TYPE);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, CREATED_AT);
  impl_i64_update!(
    set_last_modified,
    set_last_modified_if_not_none,
    LAST_MODIFIED
  );

  pub fn set_type_options(self, type_options: TypeOptions) -> Self {
    let map_ref = self
      .map_ref
      .get_or_create_map_with_txn(self.txn, FIELD_TYPE_OPTION);
    type_options.fill_map_ref(self.txn, &map_ref);
    self
  }

  /// Update type options
  pub fn update_type_options(self, f: impl FnOnce(TypeOptionsUpdate)) -> Self {
    if let Some(map_ref) = self.map_ref.get_map_with_txn(self.txn, FIELD_TYPE_OPTION) {
      let update = TypeOptionsUpdate::new(self.txn, &map_ref);
      f(update);
    }
    self
  }

  /// Set type option data for a field type
  /// If type option data is None, the type option data will be removed if it exists.
  /// If type option data is Some, the type option data will be updated or inserted.
  pub fn set_type_option(self, field_type: i64, type_option_data: Option<TypeOptionData>) -> Self {
    let map_ref = self
      .map_ref
      .get_or_create_map_with_txn(self.txn, FIELD_TYPE_OPTION);

    let update = TypeOptionsUpdate::new(self.txn, &map_ref);
    if let Some(type_option_data) = type_option_data {
      update.insert(&field_type.to_string(), type_option_data);
    } else {
      update.remove(&field_type.to_string());
    }
    self
  }

  pub fn done(self) -> Option<Field> {
    field_from_map_ref(self.map_ref, self.txn)
  }
}

const FIELD_ID: &str = "id";
const FIELD_NAME: &str = "name";
const FIELD_TYPE: &str = "ty";
const FIELD_TYPE_OPTION: &str = "type_option";
const FIELD_VISIBILITY: &str = "visibility";
const FIELD_WIDTH: &str = "width";
const FIELD_PRIMARY: &str = "is_primary";
const CREATED_AT: &str = "created_at";
const LAST_MODIFIED: &str = "last_modified";

/// Get field id from a value
pub fn field_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref = value.to_ymap()?;
  map_ref.get_str_with_txn(txn, FIELD_ID)
}

/// Get primary field id from a value
pub fn primary_field_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref = value.to_ymap()?;
  let is_primary = map_ref.get_bool_with_txn(txn, FIELD_PRIMARY)?;
  if is_primary {
    map_ref.get_str_with_txn(txn, FIELD_ID)
  } else {
    None
  }
}

/// Get field from a [YrsValue]
pub fn field_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Field> {
  let map_ref = value.to_ymap()?;
  field_from_map_ref(&map_ref, txn)
}

/// Get field from a [MapRef]
pub fn field_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Field> {
  let id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let name = map_ref
    .get_str_with_txn(txn, FIELD_NAME)
    .unwrap_or_default();

  let visibility = map_ref
    .get_bool_with_txn(txn, FIELD_VISIBILITY)
    .unwrap_or(true);

  let width = map_ref.get_i64_with_txn(txn, FIELD_WIDTH).unwrap_or(120);

  let type_options = map_ref
    .get_map_with_txn(txn, FIELD_TYPE_OPTION)
    .map(|map_ref| TypeOptions::from_map_ref(txn, map_ref))
    .unwrap_or_default();

  let field_type = map_ref.get_i64_with_txn(txn, FIELD_TYPE)?;

  let is_primary = map_ref
    .get_bool_with_txn(txn, FIELD_PRIMARY)
    .unwrap_or(false);

  Some(Field {
    id,
    name,
    field_type,
    visibility,
    width,
    type_options,
    is_primary,
  })
}
