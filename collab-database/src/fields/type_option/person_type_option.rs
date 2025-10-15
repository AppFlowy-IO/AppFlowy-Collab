use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  fields::{TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder},
  rows::Cell,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersonTypeOption {
  pub is_single_select: bool,
  pub fill_with_creator: bool,
  pub disable_notification: bool,
  pub persons: Vec<DatabasePerson>,
}

impl TypeOptionCellReader for PersonTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    serde_json::to_value(self.stringify_cell(cell)).unwrap_or(Value::Null)
  }

  fn stringify_cell(&self, cell_data: &Cell) -> String {
    serde_json::to_string(cell_data).unwrap_or_default()
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    text.to_string()
  }
}

impl TypeOptionCellWriter for PersonTypeOption {
  fn convert_json_to_cell(&self, _json_value: Value) -> Cell {
    Cell::new()
  }
}

impl From<TypeOptionData> for PersonTypeOption {
  fn from(data: TypeOptionData) -> Self {
    data
      .get_as::<String>("content")
      .map(|s| serde_json::from_str::<PersonTypeOption>(&s).unwrap_or_default())
      .unwrap_or_default()
  }
}

impl From<PersonTypeOption> for TypeOptionData {
  fn from(data: PersonTypeOption) -> Self {
    let content = serde_json::to_string(&data).unwrap_or_default();
    TypeOptionDataBuilder::from([("content".into(), content.into())])
  }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DatabasePerson {
  pub id: String,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub avatar_url: Option<String>,
}
