use crate::entity::FieldType;
use crate::fields::time_type_option::{DateFormat, TimeFormat};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TimestampTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub include_time: bool,
  pub field_type: FieldType,
}

impl TimestampTypeOption {
  pub fn new(field_type: FieldType, include_time: bool) -> Self {
    Self {
      date_format: DateFormat::default(),
      time_format: TimeFormat::default(),
      include_time,
      field_type,
    }
  }
}
