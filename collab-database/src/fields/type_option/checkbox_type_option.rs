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
  fn write_json(&self, value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::Checkbox);
    if let Some(data) = match value {
      Value::String(s) => Some(s),
      Value::Bool(b) => Some(b.to_string()),
      Value::Number(n) => Some(n.to_string()),
      _ => None,
    } {
      cell.insert(CELL_DATA.into(), bool_from_str(&data).into());
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
