use crate::entity::FieldType;
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;

use crate::template::util::{ToCellString, TypeOptionCellData};
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimestampCellData {
  pub timestamp: Option<i64>,
}

impl TypeOptionCellData for TimestampCellData {
  fn is_cell_empty(&self) -> bool {
    self.timestamp.is_none()
  }
}

impl TimestampCellData {
  pub fn new<T: Into<Option<i64>>>(timestamp: T) -> Self {
    Self {
      timestamp: timestamp.into(),
    }
  }

  pub fn to_cell<T: Into<FieldType>>(&self, field_type: T) -> Cell {
    let data: TimestampCellDataWrapper = (field_type.into(), self.clone()).into();
    data.into()
  }
}

impl ToCellString for TimestampCellData {
  fn to_cell_string(&self) -> String {
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
