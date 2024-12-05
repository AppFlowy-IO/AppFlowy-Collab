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
use crate::fields::checklist_type_option::ChecklistTypeOption;
use crate::fields::date_type_option::{DateTypeOption, TimeTypeOption};
use crate::fields::media_type_option::MediaTypeOption;
use crate::fields::number_type_option::NumberTypeOption;
use crate::fields::relation_type_option::RelationTypeOption;
use crate::fields::select_type_option::{MultiSelectTypeOption, SingleSelectTypeOption};
use crate::fields::summary_type_option::SummarizationTypeOption;
use crate::fields::timestamp_type_option::TimestampTypeOption;
use crate::fields::translate_type_option::TranslateTypeOption;
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

/// [TypeOptionCellReader] is a trait that provides methods to read cell data based on the field type.
/// It's used to convert the raw cell data into a human-readable text representation.
pub trait TypeOptionCellReader {
  /// Returns the cell data as a JSON value.
  ///
  /// The type of the returned value depends on the field type:
  /// - **Single Select**: Returns an array of [SelectOption].
  /// - **RichText**: Returns a string.
  /// - Other field types: Returns appropriate JSON values, such as objects or arrays.
  fn json_cell(&self, cell: &Cell) -> serde_json::Value;

  /// Returns a human-readable text representation of the cell.
  ///
  /// For certain field types, the raw cell data might require formatting:
  /// - **Single/Multi-Select**: The raw data may contain IDs as a comma-separated string.
  ///   Calling `stringify_cell` will convert these IDs into a list of option names, separated by commas.
  fn stringify_cell(&self, cell: &Cell) -> String {
    match cell.get_as::<String>(CELL_DATA) {
      None => "".to_string(),
      Some(s) => Self::convert_raw_cell_data(self, &s),
    }
  }

  /// Returns the numeric value of the cell. If the value is not numeric, returns `None`.
  /// Currently, it's used to calculate the sum of the numeric cell values.
  fn numeric_cell(&self, cell: &Cell) -> Option<f64>;

  /// Convert the value stored in given key:[CELL_DATA] into a readable text
  fn convert_raw_cell_data(&self, cell_data: &str) -> String;
}

/// [TypeOptionCellWriter] is a trait that provides methods to write [serde_json::Value] into a cell.
/// Different field types have their own implementation about how to convert [serde_json::Value] into [Cell].
pub trait TypeOptionCellWriter {
  /// Convert json value into a cell
  /// Different type option has its own implementation about how to convert [serde_json::Value]
  /// into [Cell]
  fn convert_json_to_cell(&self, json_value: serde_json::Value) -> Cell;
}
pub fn type_option_cell_writer(
  type_option_data: TypeOptionData,
  field_type: &FieldType,
) -> Box<dyn TypeOptionCellWriter> {
  match field_type {
    FieldType::RichText => Box::new(RichTextTypeOption::from(type_option_data)),
    FieldType::Number => Box::new(NumberTypeOption::from(type_option_data)),
    FieldType::DateTime => Box::new(DateTypeOption::from(type_option_data)),
    FieldType::SingleSelect => Box::new(SingleSelectTypeOption::from(type_option_data)),
    FieldType::MultiSelect => Box::new(MultiSelectTypeOption::from(type_option_data)),
    FieldType::Checkbox => Box::new(CheckboxTypeOption::from(type_option_data)),
    FieldType::URL => Box::new(URLTypeOption::from(type_option_data)),
    FieldType::Time => Box::new(TimeTypeOption::from(type_option_data)),
    FieldType::Media => Box::new(MediaTypeOption::from(type_option_data)),
    FieldType::Checklist => Box::new(ChecklistTypeOption::from(type_option_data)),
    FieldType::LastEditedTime => Box::new(TimestampTypeOption::from(type_option_data)),
    FieldType::CreatedTime => Box::new(TimestampTypeOption::from(type_option_data)),
    FieldType::Relation => Box::new(RelationTypeOption::from(type_option_data)),
    FieldType::Summary => Box::new(SummarizationTypeOption::from(type_option_data)),
    FieldType::Translate => Box::new(TranslateTypeOption::from(type_option_data)),
  }
}

pub fn type_option_cell_reader(
  type_option_data: TypeOptionData,
  field_type: &FieldType,
) -> Box<dyn TypeOptionCellReader> {
  match field_type {
    FieldType::RichText => Box::new(RichTextTypeOption::from(type_option_data)),
    FieldType::Number => Box::new(NumberTypeOption::from(type_option_data)),
    FieldType::DateTime => Box::new(DateTypeOption::from(type_option_data)),
    FieldType::SingleSelect => Box::new(SingleSelectTypeOption::from(type_option_data)),
    FieldType::MultiSelect => Box::new(MultiSelectTypeOption::from(type_option_data)),
    FieldType::Checkbox => Box::new(CheckboxTypeOption::from(type_option_data)),
    FieldType::URL => Box::new(URLTypeOption::from(type_option_data)),
    FieldType::Time => Box::new(TimeTypeOption::from(type_option_data)),
    FieldType::Media => Box::new(MediaTypeOption::from(type_option_data)),
    FieldType::Checklist => Box::new(ChecklistTypeOption::from(type_option_data)),
    FieldType::LastEditedTime => Box::new(TimestampTypeOption::from(type_option_data)),
    FieldType::CreatedTime => Box::new(TimestampTypeOption::from(type_option_data)),
    FieldType::Relation => Box::new(RelationTypeOption::from(type_option_data)),
    FieldType::Summary => Box::new(SummarizationTypeOption::from(type_option_data)),
    FieldType::Translate => Box::new(TranslateTypeOption::from(type_option_data)),
  }
}
