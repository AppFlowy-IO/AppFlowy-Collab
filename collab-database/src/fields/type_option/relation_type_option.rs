use super::{TypeOptionData, TypeOptionDataBuilder};
use crate::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::rows::Cell;
use crate::template::relation_parse::RelationCellData;
use crate::template::util::ToCellString;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::str::FromStr;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationTypeOption {
  pub database_id: String,
}

impl From<TypeOptionData> for RelationTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let database_id: String = data.get_as("database_id").unwrap_or_default();
    Self { database_id }
  }
}

impl From<RelationTypeOption> for TypeOptionData {
  fn from(data: RelationTypeOption) -> Self {
    TypeOptionDataBuilder::from([("database_id".into(), data.database_id.into())])
  }
}

impl TypeOptionCellReader for RelationTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = RelationCellData::from(cell);
    json!(cell_data)
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    let cell_data = RelationCellData::from_str(cell_data).unwrap_or_default();
    cell_data.to_cell_string()
  }
}

impl TypeOptionCellWriter for RelationTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<RelationCellData>(json_value).unwrap_or_default();
    Cell::from(cell_data)
  }
}
