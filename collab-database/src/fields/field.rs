use serde::{Deserialize, Serialize};

use collab::preclude::{Any, Map, MapExt, MapRef, ReadTxn, TransactionMut, YrsValue};

use crate::database::gen_field_id;
use crate::entity::{FieldType, default_type_option_data_from_type};
use crate::fields::{TypeOptionData, TypeOptions, TypeOptionsUpdate};
use crate::{impl_bool_update, impl_i64_update, impl_str_update};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Field {
  pub id: String,
  pub name: String,
  pub field_type: i64,
  #[serde(default = "DEFAULT_ICON_VALUE")]
  pub icon: String,
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
      is_primary,
      ..Default::default()
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

  pub fn from_field_type(name: &str, field_type: FieldType, is_primary: bool) -> Self {
    let new_field = Self {
      id: gen_field_id(),
      name: name.to_string(),
      field_type: field_type.into(),
      is_primary,
      ..Default::default()
    };
    new_field.with_type_option_data(field_type, default_type_option_data_from_type(field_type))
  }

  pub fn get_type_option<T: From<TypeOptionData>>(&self, type_id: impl ToString) -> Option<T> {
    let type_option_data = self.type_options.get(&type_id.to_string())?.clone();
    Some(T::from(type_option_data))
  }

  pub fn get_any_type_option(&self, type_id: impl ToString) -> Option<TypeOptionData> {
    self.type_options.get(&type_id.to_string()).cloned()
  }
}

const DEFAULT_ICON_VALUE: fn() -> String = || "".to_string();
const DEFAULT_IS_PRIMARY_VALUE: fn() -> bool = || false;

pub struct FieldBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> FieldBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
    map_ref.try_update(txn, FIELD_ID, id);
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
  impl_str_update!(set_icon, set_icon_if_not_none, FIELD_ICON);
  impl_bool_update!(set_primary, set_primary_if_not_none, FIELD_PRIMARY);
  impl_i64_update!(set_field_type, set_field_type_if_not_none, FIELD_TYPE);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, CREATED_AT);
  impl_i64_update!(
    set_last_modified,
    set_last_modified_if_not_none,
    LAST_MODIFIED
  );

  pub fn set_type_options(self, type_options: TypeOptions) -> Self {
    let map_ref: MapRef = self.map_ref.get_or_init(self.txn, FIELD_TYPE_OPTION);
    type_options.fill_map_ref(self.txn, &map_ref);
    self
  }

  /// Update type options
  pub fn update_type_options(self, f: impl FnOnce(TypeOptionsUpdate)) -> Self {
    if let Some(map_ref) = self.map_ref.get_with_txn(self.txn, FIELD_TYPE_OPTION) {
      let update = TypeOptionsUpdate::new(self.txn, &map_ref);
      f(update);
    }
    self
  }

  /// Set type option data for a field type
  /// If type option data is None, the type option data will be removed if it exists.
  /// If type option data is Some, the type option data will be updated or inserted.
  pub fn set_type_option(self, field_type: i64, type_option_data: Option<TypeOptionData>) -> Self {
    let map_ref: MapRef = self.map_ref.get_or_init(self.txn, FIELD_TYPE_OPTION);

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
const FIELD_ICON: &str = "icon";
const FIELD_TYPE: &str = "ty";
const FIELD_TYPE_OPTION: &str = "type_option";
const FIELD_PRIMARY: &str = "is_primary";
const CREATED_AT: &str = "created_at";
const LAST_MODIFIED: &str = "last_modified";

/// Get field id from a value
pub fn field_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref: MapRef = value.cast().ok()?;
  map_ref.get(txn, FIELD_ID).and_then(|v| v.cast().ok())
}

/// Get primary field id from a value
pub fn primary_field_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref: MapRef = value.cast().ok()?;
  let is_primary: bool = map_ref.get(txn, FIELD_PRIMARY)?.cast().ok()?;
  if is_primary {
    map_ref.get(txn, FIELD_ID)?.cast().ok()
  } else {
    None
  }
}

/// Get field from a [YrsValue]
pub fn field_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Field> {
  let map_ref: MapRef = value.cast().ok()?;
  field_from_map_ref(&map_ref, txn)
}

/// Get field from a [MapRef]
pub fn field_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Field> {
  let id: String = map_ref.get_with_txn(txn, FIELD_ID)?;
  let name: String = map_ref.get_with_txn(txn, FIELD_NAME).unwrap_or_default();
  let icon: String = map_ref.get_with_txn(txn, FIELD_ICON).unwrap_or_default();

  let type_options: TypeOptions = map_ref
    .get_with_txn(txn, FIELD_TYPE_OPTION)
    .map(|map_ref: MapRef| TypeOptions::from_map_ref(txn, map_ref))
    .unwrap_or_default();

  let field_type: i64 = map_ref.get_with_txn(txn, FIELD_TYPE)?;

  let is_primary: bool = map_ref.get_with_txn(txn, FIELD_PRIMARY).unwrap_or(false);

  Some(Field {
    id,
    name,
    icon,
    field_type,
    type_options,
    is_primary,
  })
}
