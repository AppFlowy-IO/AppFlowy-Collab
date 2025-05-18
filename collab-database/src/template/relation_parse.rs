use crate::entity::FieldType;
use std::str::FromStr;

use crate::error::DatabaseError;
use crate::rows::{Cell, RowId, new_cell_builder};
use crate::template::entity::CELL_DATA;
use crate::template::util::{ToCellString, TypeOptionCellData};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yrs::Any;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationCellData {
  pub row_ids: Vec<RowId>,
}

impl FromStr for RelationCellData {
  type Err = DatabaseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.is_empty() {
      return Ok(RelationCellData { row_ids: vec![] });
    }

    let ids = s
      .split(", ")
      .map(|id| id.to_string().into())
      .collect::<Vec<_>>();

    Ok(RelationCellData { row_ids: ids })
  }
}

impl TypeOptionCellData for RelationCellData {
  fn is_cell_empty(&self) -> bool {
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

impl From<RelationCellData> for Cell {
  fn from(value: RelationCellData) -> Self {
    let data = Any::Array(Arc::from(
      value
        .row_ids
        .into_iter()
        .map(|id| Any::String(Arc::from(id.to_string())))
        .collect::<Vec<_>>(),
    ));
    let mut cell = new_cell_builder(FieldType::Relation);
    cell.insert(CELL_DATA.into(), data);
    cell
  }
}

impl ToCellString for RelationCellData {
  fn to_cell_string(&self) -> String {
    self
      .row_ids
      .iter()
      .map(|id| id.to_string())
      .collect::<Vec<_>>()
      .join(", ")
  }
}
