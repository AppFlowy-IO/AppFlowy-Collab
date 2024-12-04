use crate::entity::FieldType;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RichTextTypeOption;

impl TypeOptionCellReader for RichTextTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    json!(self.stringify_cell(cell))
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    self.stringify_cell(cell).parse().ok()
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    text.to_string()
  }
}

impl TypeOptionCellWriter for RichTextTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::RichText);
    cell.insert(CELL_DATA.into(), json_value.to_string().into());
    cell
  }
}

impl From<TypeOptionData> for RichTextTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    RichTextTypeOption
  }
}

impl From<RichTextTypeOption> for TypeOptionData {
  fn from(_data: RichTextTypeOption) -> Self {
    TypeOptionDataBuilder::new()
  }
}
