use crate::entity::FieldType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DateTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub timezone_id: String,
}

impl DateTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize, Default)]
pub enum TimeFormat {
  TwelveHour = 0,
  #[default]
  TwentyFourHour = 1,
}
#[derive(Clone, Debug, Copy, Serialize, Deserialize, Default)]
pub enum DateFormat {
  Local = 0,
  US = 1,
  ISO = 2,
  #[default]
  Friendly = 3,
  DayMonthYear = 4,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
