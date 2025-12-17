use super::{TypeOptionData, TypeOptionDataBuilder};
use crate::database::entity::FieldType;
use crate::database::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::database::rows::{Cell, new_cell_builder};
use crate::database::template::entity::CELL_DATA;
use crate::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_repr::{Deserialize_repr, Serialize_repr};
use yrs::Any;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i64)]
pub enum RollupDisplayMode {
  #[default]
  Calculated = 0,
  OriginalList = 1,
  UniqueList = 2,
}

impl From<i64> for RollupDisplayMode {
  fn from(value: i64) -> Self {
    match value {
      0 => RollupDisplayMode::Calculated,
      1 => RollupDisplayMode::OriginalList,
      2 => RollupDisplayMode::UniqueList,
      _ => RollupDisplayMode::Calculated,
    }
  }
}

impl From<RollupDisplayMode> for i64 {
  fn from(value: RollupDisplayMode) -> Self {
    value as i64
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupTypeOption {
  pub relation_field_id: String,
  pub target_field_id: String,
  pub calculation_type: i64,
  pub show_as: RollupDisplayMode,
  #[serde(default)]
  pub condition_value: String,
}

impl Default for RollupTypeOption {
  fn default() -> Self {
    Self {
      relation_field_id: String::new(),
      target_field_id: String::new(),
      // Default to Count, which is applicable across all field types.
      calculation_type: 5,
      show_as: RollupDisplayMode::Calculated,
      condition_value: String::new(),
    }
  }
}

impl From<TypeOptionData> for RollupTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let relation_field_id: String = data.get_as("relation_field_id").unwrap_or_default();
    let target_field_id: String = data.get_as("target_field_id").unwrap_or_default();
    let calculation_type: i64 = data.get_as("calculation_type").unwrap_or(5);
    let show_as: i64 = data.get_as("show_as").unwrap_or(0);
    let condition_value: String = data.get_as("condition_value").unwrap_or_default();
    Self {
      relation_field_id,
      target_field_id,
      calculation_type,
      show_as: show_as.into(),
      condition_value,
    }
  }
}

impl From<RollupTypeOption> for TypeOptionData {
  fn from(data: RollupTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      (
        "relation_field_id".into(),
        Any::String(data.relation_field_id.into()),
      ),
      (
        "target_field_id".into(),
        Any::String(data.target_field_id.into()),
      ),
      (
        "calculation_type".into(),
        Any::BigInt(data.calculation_type),
      ),
      ("show_as".into(), Any::BigInt(i64::from(data.show_as))),
      (
        "condition_value".into(),
        Any::String(data.condition_value.into()),
      ),
    ])
  }
}

impl TypeOptionCellReader for RollupTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    json!(self.stringify_cell(cell))
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    self.stringify_cell(cell).parse().ok()
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    cell_data.to_string()
  }
}

impl TypeOptionCellWriter for RollupTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::Rollup);
    match json_value {
      Value::String(value_str) => {
        cell.insert(CELL_DATA.into(), value_str.into());
      },
      _ => {
        cell.insert(CELL_DATA.into(), json_value.to_string().into());
      },
    }
    cell
  }
}
