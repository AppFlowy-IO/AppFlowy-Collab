use crate::entity::FieldType;

use crate::rows::{new_cell_builder, Cell, RowId};
use crate::template::entity::CELL_DATA;
use crate::template::util::TypeOptionCellData;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yrs::Any;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationCellData {
  pub row_ids: Vec<RowId>,
}

impl TypeOptionCellData for RelationCellData {
  fn is_empty(&self) -> bool {
    self.row_ids.is_empty()
  }
}

impl From<&Cell> for RelationCellData {
  fn from(value: &Cell) -> Self {
    let row_ids = match value.get(CELL_DATA) {
      Some(Any::Array(array)) => array
        .iter()
        .flat_map(|item| {
          if let Any::String(string) = item {
            Some(RowId::from(string.clone().to_string()))
          } else {
            None
          }
        })
        .collect(),
      _ => vec![],
    };
    Self { row_ids }
  }
}

impl From<&RelationCellData> for Cell {
  fn from(value: &RelationCellData) -> Self {
    let data = Any::Array(Arc::from(
      value
        .row_ids
        .clone()
        .into_iter()
        .map(|id| Any::String(Arc::from(id.to_string())))
        .collect::<Vec<_>>(),
    ));
    let mut cell = new_cell_builder(FieldType::Relation);
    cell.insert(CELL_DATA.into(), data);
    cell
  }
}

impl From<&str> for RelationCellData {
  fn from(s: &str) -> Self {
    if s.is_empty() {
      return RelationCellData { row_ids: vec![] };
    }

    let ids = s
      .split(", ")
      .map(|id| id.to_string().into())
      .collect::<Vec<_>>();

    RelationCellData { row_ids: ids }
  }
}
