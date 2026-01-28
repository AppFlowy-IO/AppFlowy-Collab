use crate::database::entity::FieldType;
use crate::database::rows::{Cell, new_cell_builder};
use crate::database::template::entity::CELL_DATA;
use crate::database::template::util::{ToCellString, TypeOptionCellData};
use crate::util::AnyMapExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeCellData(pub Option<i64>);

impl TypeOptionCellData for TimeCellData {
  fn is_cell_empty(&self) -> bool {
    self.0.is_none()
  }
}

impl From<&Cell> for TimeCellData {
  fn from(cell: &Cell) -> Self {
    Self(
      cell
        .get_as::<String>(CELL_DATA)
        .and_then(|data| data.parse::<i64>().ok()),
    )
  }
}

impl std::convert::From<&str> for TimeCellData {
  fn from(s: &str) -> Self {
    Self(s.trim().to_string().parse::<i64>().ok())
  }
}

impl ToCellString for TimeCellData {
  fn to_cell_string(&self) -> String {
    if let Some(time) = self.0 {
      time.to_string()
    } else {
      "".to_string()
    }
  }
}

impl From<&TimeCellData> for Cell {
  fn from(data: &TimeCellData) -> Self {
    let mut cell = new_cell_builder(FieldType::Time);
    cell.insert(CELL_DATA.into(), data.to_cell_string().into());
    cell
  }
}
