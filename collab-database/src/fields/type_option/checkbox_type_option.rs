use std::sync::Arc;

use crate::entity::FieldType;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckboxTypeOption;

impl CheckboxTypeOption {
  pub fn new() -> Self {
    Self
  }
}

impl TypeOptionCellReader for CheckboxTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    match cell.get_as::<Arc<str>>(CELL_DATA) {
      None => false.into(),
      Some(s) => bool_from_str(&s).into(),
    }
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    let cell_data = cell.get_as::<String>(CELL_DATA)?;
    if bool_from_str(&cell_data) {
      Some(1.0)
    } else {
      Some(0.0)
    }
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    text.to_string()
  }
}

impl TypeOptionCellWriter for CheckboxTypeOption {
  fn convert_json_to_cell(&self, value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::Checkbox);
    let checked = match value {
      Value::String(s) => bool_from_str(&s),
      Value::Bool(b) => b,
      Value::Number(n) => n.as_i64().unwrap_or(0) > 0,
      _ => false,
    };
    cell.insert(CELL_DATA.into(), checked.to_string().into());
    cell
  }
}

impl From<CheckboxTypeOption> for TypeOptionData {
  fn from(_data: CheckboxTypeOption) -> Self {
    TypeOptionDataBuilder::new()
  }
}

impl From<TypeOptionData> for CheckboxTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    CheckboxTypeOption
  }
}

fn bool_from_str(s: &str) -> bool {
  let lower_case_str: &str = &s.to_lowercase();
  match lower_case_str {
    "1" | "true" | "yes" => true,
    "0" | "false" | "no" => false,
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_checkbox_type_option_json_cell() {
    let option = CheckboxTypeOption::new();
    let mut cell = new_cell_builder(FieldType::Checkbox);
    cell.insert(CELL_DATA.into(), "true".into());

    // Convert cell to JSON
    let value = option.json_cell(&cell);
    assert_eq!(value, Value::Bool(true));

    // Test with empty data
    let empty_cell = new_cell_builder(FieldType::Checkbox);
    let empty_value = option.json_cell(&empty_cell);
    assert_eq!(empty_value, Value::Bool(false));
  }

  #[test]
  fn test_checkbox_type_option_numeric_cell() {
    let option = CheckboxTypeOption::new();

    let mut true_cell = new_cell_builder(FieldType::Checkbox);
    true_cell.insert(CELL_DATA.into(), "true".into());
    assert_eq!(option.numeric_cell(&true_cell), Some(1.0));

    let mut false_cell = new_cell_builder(FieldType::Checkbox);
    false_cell.insert(CELL_DATA.into(), "false".into());
    assert_eq!(option.numeric_cell(&false_cell), Some(0.0));

    let mut invalid_cell = new_cell_builder(FieldType::Checkbox);
    invalid_cell.insert(CELL_DATA.into(), "invalid".into());
    assert_eq!(option.numeric_cell(&invalid_cell), Some(0.0));
  }

  #[test]
  fn test_checkbox_type_option_write_json() {
    let option = CheckboxTypeOption::new();

    // Write a string
    let value = Value::String("true".to_string());
    let cell = option.convert_json_to_cell(value);
    assert_eq!(cell.get_as::<String>(CELL_DATA).unwrap(), "true");

    // Write a boolean
    let value = Value::Bool(true);
    let cell = option.convert_json_to_cell(value);
    assert_eq!(cell.get_as::<String>(CELL_DATA).unwrap(), "true");

    // Write a number
    let value = Value::Number(1.into());
    let cell = option.convert_json_to_cell(value);
    assert_eq!(cell.get_as::<String>(CELL_DATA).unwrap(), "true");
  }

  #[test]
  fn test_checkbox_type_option_raw_conversion() {
    let option = CheckboxTypeOption::new();
    assert_eq!(
      option.convert_raw_cell_data("raw data"),
      "raw data".to_string()
    );
  }

  #[test]
  fn test_bool_from_str() {
    assert!(bool_from_str("true"));
    assert!(bool_from_str("1"));
    assert!(bool_from_str("yes"));

    assert!(!bool_from_str("false"));
    assert!(!bool_from_str("0"));
    assert!(!bool_from_str("no"));

    // Invalid inputs default to false
    assert!(!bool_from_str("invalid"));
    assert!(!bool_from_str(""));
  }

  #[test]
  fn checkbox_cell_to_serde() {
    let checkbox_type_option = CheckboxTypeOption::new();
    let cell_writer: Box<dyn TypeOptionCellReader> = Box::new(checkbox_type_option);
    {
      let mut cell: Cell = new_cell_builder(FieldType::Checkbox);
      cell.insert(CELL_DATA.into(), "Yes".into());
      let serde_val = cell_writer.json_cell(&cell);
      assert_eq!(serde_val, Value::Bool(true));
    }
  }

  #[test]
  fn number_serde_to_cell() {
    let checkbox_type_option = CheckboxTypeOption;
    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(checkbox_type_option);
    {
      // empty string
      let cell: Cell = cell_writer.convert_json_to_cell(Value::String("".to_string()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "false");
    }
    {
      // "yes" in any case
      let cell: Cell = cell_writer.convert_json_to_cell(Value::String("yEs".to_string()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "true");
    }
    {
      // bool
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Bool(true));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "true");
    }
    {
      // js number
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Number(1.into()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "true");
    }
    {
      // js null
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Null);
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "false");
    }
  }
}
