use super::{TypeOptionData, TypeOptionDataBuilder};
use crate::entity::FieldType;
use crate::fields::select_type_option::SELECTION_IDS_SEPARATOR;
use crate::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::rows::{new_cell_builder, Cell};
use crate::template::check_list_parse::ChecklistCellData;
use crate::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChecklistTypeOption;

impl From<TypeOptionData> for ChecklistTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    Self
  }
}

impl From<ChecklistTypeOption> for TypeOptionData {
  fn from(_data: ChecklistTypeOption) -> Self {
    TypeOptionDataBuilder::default()
  }
}

impl TypeOptionCellReader for ChecklistTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = ChecklistCellData::from(cell);
    json!(cell_data)
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    let cell_data = serde_json::from_str::<ChecklistCellData>(cell_data).unwrap_or_default();
    cell_data
      .options
      .into_iter()
      .map(|option| option.name)
      .collect::<Vec<_>>()
      .join(SELECTION_IDS_SEPARATOR)
  }
}

impl TypeOptionCellWriter for ChecklistTypeOption {
  fn write_json(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<ChecklistCellData>(json_value).unwrap_or_default();
    cell_data.into()
  }
}

impl From<&Cell> for ChecklistCellData {
  fn from(cell: &Cell) -> Self {
    cell
      .get_as::<String>(CELL_DATA)
      .map(|data| serde_json::from_str::<ChecklistCellData>(&data).unwrap_or_default())
      .unwrap_or_default()
  }
}

impl From<ChecklistCellData> for Cell {
  fn from(cell_data: ChecklistCellData) -> Self {
    let data = serde_json::to_string(&cell_data).unwrap_or_default();
    let mut cell = new_cell_builder(FieldType::Checklist);
    cell.insert(CELL_DATA.into(), data.into());
    cell
  }
}
