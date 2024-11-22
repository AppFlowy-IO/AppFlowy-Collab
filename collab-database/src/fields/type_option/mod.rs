pub mod checkbox_type_option;
pub mod checklist_type_option;
pub mod date_type_option;
pub mod media_type_option;
pub mod number_type_option;
pub mod relation_type_option;
pub mod select_type_option;
pub mod summary_type_option;
pub mod text_type_option;
pub mod timestamp_type_option;
pub mod translate_type_option;
pub mod url_type_option;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::entity::FieldType;
use crate::fields::date_type_option::{DateTypeOption, TimeTypeOption};
use crate::fields::media_type_option::MediaTypeOption;
use crate::fields::number_type_option::NumberTypeOption;
use crate::fields::select_type_option::{MultiSelectTypeOption, SingleSelectTypeOption};
use crate::fields::type_option::checkbox_type_option::CheckboxTypeOption;
use crate::fields::type_option::text_type_option::RichTextTypeOption;
use crate::fields::url_type_option::URLTypeOption;
use crate::rows::Cell;
use crate::template::entity::CELL_DATA;
use collab::preclude::{Any, FillRef, Map, MapRef, ReadTxn, ToJson, TransactionMut};
use collab::util::{AnyExt, AnyMapExt};
use serde::{Deserialize, Serialize};

/// It's used to store lists of field's type option data
/// The key is the [FieldType] string representation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeOptions(HashMap<String, TypeOptionData>);

impl TypeOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, TypeOptionData> {
    self.0
  }

  /// Returns a new instance of [TypeOptions] from a [MapRef]
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let Some(type_option_data) = v.to_json(txn).into_map() {
        this.insert(k.to_string(), type_option_data);
      }
    });
    this
  }

  /// Fill the [MapRef] with the [TypeOptions] data
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      let update = TypeOptionsUpdate::new(txn, map_ref);
      update.insert(&k, v);
    });
  }
}

impl Deref for TypeOptions {
  type Target = HashMap<String, TypeOptionData>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for TypeOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

pub struct TypeOptionsUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> TypeOptionsUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  /// Insert a new cell's key/value into the [TypeOptionData]
  pub fn insert<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map: MapRef = self.map_ref.get_or_init(self.txn, key);
    Any::from(value).fill(self.txn, &type_option_map).unwrap();
    self
  }

  /// Override the existing cell's key/value contained in the [TypeOptionData]
  /// It will create the type option if it's not exist
  pub fn update<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map: MapRef = self.map_ref.get_or_init(self.txn, key);
    Any::from(value).fill(self.txn, &type_option_map).unwrap();
    self
  }

  /// Remove the cell's key/value from the [TypeOptionData]
  pub fn remove(self, key: &str) -> Self {
    self.map_ref.remove(self.txn, key);
    self
  }
}

pub type TypeOptionData = HashMap<String, Any>;
pub type TypeOptionDataBuilder = HashMap<String, Any>;
pub type TypeOptionUpdate = MapRef;

/// It's used to parse each cell into readable text
pub trait StringifyTypeOption {
  fn stringify_cell(&self, cell: &Cell) -> String {
    match cell.get_as::<String>(CELL_DATA) {
      None => "".to_string(),
      Some(s) => Self::stringify_text(self, &s),
    }
  }
  fn stringify_text(&self, text: &str) -> String;
}
pub fn stringify_type_option(
  type_option_data: TypeOptionData,
  field_type: &FieldType,
) -> Option<Box<dyn StringifyTypeOption>> {
  match field_type {
    FieldType::RichText => Some(Box::new(RichTextTypeOption::from(type_option_data))),
    FieldType::Number => Some(Box::new(NumberTypeOption::from(type_option_data))),
    FieldType::DateTime => Some(Box::new(DateTypeOption::from(type_option_data))),
    FieldType::SingleSelect => Some(Box::new(SingleSelectTypeOption::from(type_option_data))),
    FieldType::MultiSelect => Some(Box::new(MultiSelectTypeOption::from(type_option_data))),
    FieldType::Checkbox => Some(Box::new(CheckboxTypeOption::from(type_option_data))),
    FieldType::URL => Some(Box::new(URLTypeOption::from(type_option_data))),
    FieldType::Time => Some(Box::new(TimeTypeOption::from(type_option_data))),
    FieldType::Media => Some(Box::new(MediaTypeOption::from(type_option_data))),

    FieldType::Checklist
    | FieldType::LastEditedTime
    | FieldType::CreatedTime
    | FieldType::Relation
    | FieldType::Summary
    | FieldType::Translate => None,
  }
}
