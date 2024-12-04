use crate::entity::FieldType;
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimestampCellData {
  pub timestamp: Option<i64>,
}

impl TimestampCellData {
  pub fn new(timestamp: Option<i64>) -> Self {
    Self { timestamp }
  }

  pub fn to_cell(&self, field_type: FieldType) -> Cell {
    let data: TimestampCellDataWrapper = (field_type, self.clone()).into();
    data.into()
  }
}

impl ToString for TimestampCellData {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

impl From<&Cell> for TimestampCellData {
  fn from(cell: &Cell) -> Self {
    let timestamp = cell
      .get_as::<String>(CELL_DATA)
      .and_then(|data| data.parse::<i64>().ok());
    Self { timestamp }
  }
}

/// Wrapper for DateCellData that also contains the field type.
/// Handy struct to use when you need to convert a DateCellData to a Cell.
struct TimestampCellDataWrapper {
  data: TimestampCellData,
  field_type: FieldType,
}

impl From<(FieldType, TimestampCellData)> for TimestampCellDataWrapper {
  fn from((field_type, data): (FieldType, TimestampCellData)) -> Self {
    Self { data, field_type }
  }
}

impl From<TimestampCellDataWrapper> for Cell {
  fn from(wrapper: TimestampCellDataWrapper) -> Self {
    let (field_type, data) = (wrapper.field_type, wrapper.data);
    let timestamp_string = data.timestamp.unwrap_or_default().to_string();

    let mut cell = new_cell_builder(field_type);
    cell.insert(CELL_DATA.into(), timestamp_string.into());
    cell
  }
}
