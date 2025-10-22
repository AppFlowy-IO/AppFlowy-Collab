use crate::database::entity::FieldType;
use crate::database::rows::{Cell, new_cell_builder};
use crate::database::template::entity::CELL_DATA;
use crate::database::template::util::{ToCellString, TypeOptionCellData};
use crate::util::AnyMapExt;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SummaryCellData(pub String);

impl TypeOptionCellData for SummaryCellData {
  fn is_cell_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl std::ops::Deref for SummaryCellData {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<&Cell> for SummaryCellData {
  fn from(cell: &Cell) -> Self {
    Self(cell.get_as::<String>(CELL_DATA).unwrap_or_default())
  }
}

impl From<SummaryCellData> for Cell {
  fn from(data: SummaryCellData) -> Self {
    let mut cell = new_cell_builder(FieldType::Summary);
    cell.insert(CELL_DATA.into(), data.0.into());
    cell
  }
}

impl ToCellString for SummaryCellData {
  fn to_cell_string(&self) -> String {
    self.0.clone()
  }
}

impl AsRef<str> for SummaryCellData {
  fn as_ref(&self) -> &str {
    &self.0
  }
}
