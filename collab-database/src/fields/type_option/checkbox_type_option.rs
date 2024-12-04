use crate::entity::FieldType;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{new_cell_builder, Cell};
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
    let value = match cell.get_as::<String>(CELL_DATA) {
      None => "".to_string(),
      Some(s) => Self::convert_raw_cell_data(self, &s),
    };
    Value::String(value)
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
    if let Some(data) = match value {
      Value::String(s) => Some(s),
      Value::Bool(b) => Some(b.to_string()),
      Value::Number(n) => Some(n.to_string()),
      _ => None,
    } {
      cell.insert(CELL_DATA.into(), bool_from_str(&data).to_string().into());
    }
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
    assert_eq!(value, Value::String("true".to_string()));

    // Test with empty data
    let empty_cell = new_cell_builder(FieldType::Checkbox);
    let empty_value = option.json_cell(&empty_cell);
    assert_eq!(empty_value, Value::String("".to_string()));
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
}
