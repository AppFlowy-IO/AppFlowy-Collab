use crate::entity::FieldType;
use crate::error::DatabaseError;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;
use crate::template::util::{ToCellString, TypeOptionCellData};
use collab::preclude::Any;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yrs::encoding::serde::from_any;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct URLTypeOption {
  #[serde(default)]
  pub url: String,
  #[serde(default)]
  pub content: String,
}

impl TypeOptionCellReader for URLTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    cell.get_as::<String>(CELL_DATA).unwrap_or_default().into()
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    let cell_data = URLCellData::new(text);
    cell_data.to_cell_string()
  }
}

impl TypeOptionCellWriter for URLTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    match json_value {
      Value::String(s) => {
        let cell_data = URLCellData::new(&s);
        cell_data.into()
      },
      _ => Cell::default(),
    }
  }
}

impl From<TypeOptionData> for URLTypeOption {
  fn from(data: TypeOptionData) -> Self {
    from_any(&Any::from(data)).unwrap()
  }
}

impl From<URLTypeOption> for TypeOptionData {
  fn from(data: URLTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("url".into(), data.url.into()),
      ("content".into(), data.content.into()),
    ])
  }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct URLCellData {
  pub data: String,
}

impl TypeOptionCellData for URLCellData {
  fn is_cell_empty(&self) -> bool {
    self.data.is_empty()
  }
}

impl AsRef<str> for URLCellData {
  fn as_ref(&self) -> &str {
    &self.data
  }
}

impl URLCellData {
  pub fn new(s: &str) -> Self {
    Self {
      data: s.to_string(),
    }
  }

  pub fn to_json(&self) -> Result<String, DatabaseError> {
    serde_json::to_string(self).map_err(|err| DatabaseError::Internal(err.into()))
  }
}

impl From<&Cell> for URLCellData {
  fn from(cell: &Cell) -> Self {
    Self {
      data: cell.get_as(CELL_DATA).unwrap_or_default(),
    }
  }
}

impl From<URLCellData> for Cell {
  fn from(data: URLCellData) -> Self {
    let mut cell = new_cell_builder(FieldType::URL);
    cell.insert(CELL_DATA.into(), data.data.into());
    cell
  }
}

impl ToCellString for URLCellData {
  fn to_cell_string(&self) -> String {
    self.to_json().unwrap()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn url_cell_to_serde() {
    let url_type_option = URLTypeOption::default();
    let cell_writer: Box<dyn TypeOptionCellReader> = Box::new(url_type_option);
    {
      let mut cell: Cell = new_cell_builder(FieldType::DateTime);
      cell.insert(CELL_DATA.into(), "https://appflowy.io".into());
      let serde_val = cell_writer.json_cell(&cell);
      assert_eq!(serde_val, "https://appflowy.io");
    }
  }

  #[test]
  fn url_serde_to_cell() {
    let url_type_option = URLTypeOption::default();
    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(url_type_option);
    {
      let cell: Cell =
        cell_writer.convert_json_to_cell(Value::String("https://appflowy.io".to_string()));
      let data: String = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "https://appflowy.io");
    }
  }
}
