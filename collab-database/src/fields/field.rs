use crate::fields::TypeOptions;
use crate::{impl_any_update, impl_bool_update, impl_i64_update, impl_str_update};
use anyhow::bail;
use collab::preclude::{
  lib0Any, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Field {
  pub id: String,
  pub name: String,
  pub field_type: FieldType,
  pub visibility: bool,
  pub width: i64,
  pub type_options: TypeOptions,
  #[serde(default = "DEFAULT_IS_PRIMARY_VALUE")]
  pub is_primary: bool,
}

impl Field {
  pub fn new(id: String, name: String, field_type: FieldType, is_primary: bool) -> Self {
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
}

const DEFAULT_IS_PRIMARY_VALUE: fn() -> bool = || false;

#[derive(Eq, PartialEq, Debug, Hash, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FieldType {
  RichText = 0,
  Number = 1,
  DateTime = 2,
  SingleSelect = 3,
  MultiSelect = 4,
  Checkbox = 5,
  URL = 6,
  Checklist = 7,
}

impl Default for FieldType {
  fn default() -> Self {
    Self::RichText
  }
}

impl TryFrom<i64> for FieldType {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(FieldType::RichText),
      1 => Ok(FieldType::Number),
      2 => Ok(FieldType::DateTime),
      3 => Ok(FieldType::SingleSelect),
      4 => Ok(FieldType::MultiSelect),
      5 => Ok(FieldType::Checkbox),
      6 => Ok(FieldType::URL),
      7 => Ok(FieldType::Checklist),
      _ => bail!("Unknown field type {}", value),
    }
  }
}

impl From<FieldType> for lib0Any {
  fn from(field_type: FieldType) -> Self {
    lib0Any::BigInt(field_type as i64)
  }
}
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
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> FieldUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_name, set_name_if_not_none, FIELD_NAME);
  impl_bool_update!(set_visibility, set_visibility_if_not_none, FIELD_VISIBILITY);
  impl_bool_update!(set_primary, set_primary_if_not_none, FIELD_PRIMARY);
  impl_i64_update!(set_width, set_width_at_if_not_none, FIELD_WIDTH);
  impl_any_update!(
    set_field_type,
    set_field_type_if_not_none,
    FIELD_TYPE,
    FieldType
  );

  pub fn set_type_option(self, type_option: TypeOptions) -> Self {
    let map_ref = self
      .map_ref
      .get_or_insert_map_with_txn(self.txn, FIELD_TYPE_OPTION);
    type_option.fill_map_ref(self.txn, &map_ref);
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

pub fn field_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<String> {
  let map_ref = value.to_ymap()?;
  let map_ref_ext = MapRefExtension(&map_ref);
  map_ref_ext.get_str_with_txn(txn, FIELD_ID)
}

pub fn field_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Field> {
  let map_ref = value.to_ymap()?;
  field_from_map_ref(&map_ref, txn)
}

pub fn field_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Field> {
  let map_ref = MapRefExtension(map_ref);
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

  let field_type = map_ref
    .get_i64_with_txn(txn, FIELD_TYPE)
    .map(|value| value.try_into().ok())??;

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
