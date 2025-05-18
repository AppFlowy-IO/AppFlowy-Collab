use crate::entity::FieldType;
use crate::fields::date_type_option::{DateFormat, TimeFormat};
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::Cell;
use crate::template::timestamp_parse::TimestampCellData;
use chrono::{DateTime, Local, Offset, TimeZone};
use chrono_tz::Tz;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::str::FromStr;
use yrs::Any;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimestampTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub include_time: bool,
  pub field_type: i64,
  #[serde(default)]
  pub timezone: Option<String>,
}

impl TypeOptionCellReader for TimestampTypeOption {
  /// Return formated date and time string for the cell
  fn json_cell(&self, cell: &Cell) -> Value {
    let mut js_val: serde_json::Value = TimestampCellData::from(cell)
      .timestamp
      .and_then(|ts| DateTime::from_timestamp(ts, 0))
      .map(|dt| dt.to_rfc3339())
      .unwrap_or_default()
      .into();
    if let Some(obj) = js_val.as_object_mut() {
      obj.insert("pretty".to_string(), json!(self.stringify_cell(cell)));
    }
    js_val
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    let cell_data = TimestampCellData {
      timestamp: text.parse::<i64>().ok(),
    };

    let (date_string, time_string) = self.formatted_date_time_from_timestamp(&cell_data.timestamp);
    if self.include_time {
      format!("{} {}", date_string, time_string)
    } else {
      date_string
    }
  }
}

impl TypeOptionCellWriter for TimestampTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let filed_type = FieldType::from(self.field_type);
    let data = match json_value {
      Value::String(s) => s.parse::<i64>().ok(),
      Value::Number(n) => n.as_i64(),
      _ => None,
    };
    TimestampCellData::new(data).to_cell(filed_type)
  }
}

impl TimestampTypeOption {
  pub fn new<T: Into<i64>>(field_type: T) -> Self {
    Self {
      field_type: field_type.into(),
      include_time: true,
      ..Default::default()
    }
  }

  pub fn formatted_date_time_from_timestamp(&self, timestamp: &Option<i64>) -> (String, String) {
    if let Some(naive) = timestamp.and_then(|timestamp| {
      chrono::DateTime::from_timestamp(timestamp, 0).map(|date| date.naive_utc())
    }) {
      let offset = self
        .timezone
        .as_ref()
        .and_then(|timezone| Tz::from_str(timezone).ok())
        .map(|tz| tz.offset_from_utc_datetime(&naive).fix())
        .unwrap_or_else(|| Local::now().offset().fix());

      let date_time = DateTime::<Local>::from_naive_utc_and_offset(naive, offset);
      let fmt = self.date_format.format_str();
      let date = format!("{}", date_time.format(fmt));
      let fmt = self.time_format.format_str();
      let time = format!("{}", date_time.format(fmt));
      (date, time)
    } else {
      ("".to_owned(), "".to_owned())
    }
  }
}

impl Default for TimestampTypeOption {
  fn default() -> Self {
    Self {
      date_format: Default::default(),
      time_format: Default::default(),
      include_time: true,
      field_type: FieldType::LastEditedTime.into(),
      timezone: None,
    }
  }
}

impl From<TypeOptionData> for TimestampTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let date_format = data
      .get_as::<i64>("date_format")
      .map(DateFormat::from)
      .unwrap_or_default();
    let time_format = data
      .get_as::<i64>("time_format")
      .map(TimeFormat::from)
      .unwrap_or_default();
    let include_time = data.get_as::<bool>("include_time").unwrap_or_default();
    let field_type = data
      .get_as::<i64>("field_type")
      .map(FieldType::from)
      .unwrap_or(FieldType::LastEditedTime)
      .into();
    let timezone = data.get_as::<String>("timezone");
    Self {
      date_format,
      time_format,
      include_time,
      field_type,
      timezone,
    }
  }
}

impl From<TimestampTypeOption> for TypeOptionData {
  fn from(option: TimestampTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      (
        "date_format".into(),
        Any::BigInt(option.date_format.value()),
      ),
      (
        "time_format".into(),
        Any::BigInt(option.time_format.value()),
      ),
      ("include_time".into(), Any::Bool(option.include_time)),
      ("field_type".into(), Any::BigInt(option.field_type)),
    ])
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::template::entity::CELL_DATA;
  use serde_json::json;

  #[test]
  fn test_default_timestamp_type_option() {
    let default_option = TimestampTypeOption::default();
    assert!(default_option.include_time);
    assert_eq!(
      default_option.field_type,
      i64::from(FieldType::LastEditedTime)
    );
  }

  #[test]
  fn test_from_type_option_data() {
    let data = TypeOptionDataBuilder::from([
      ("date_format".into(), Any::BigInt(2)),
      ("time_format".into(), Any::BigInt(1)),
      ("include_time".into(), Any::Bool(false)),
      (
        "field_type".into(),
        Any::BigInt(FieldType::CreatedTime.into()),
      ),
    ]);

    let option = TimestampTypeOption::from(data);
    assert_eq!(option.date_format, DateFormat::ISO);
    assert_eq!(option.time_format, TimeFormat::TwentyFourHour);
    assert!(!option.include_time);
    assert_eq!(option.field_type, i64::from(FieldType::CreatedTime));
  }

  #[test]
  fn test_into_type_option_data() {
    let option = TimestampTypeOption {
      date_format: DateFormat::Friendly,
      time_format: TimeFormat::TwelveHour,
      include_time: true,
      field_type: FieldType::CreatedTime.into(),
      timezone: None,
    };

    let data: TypeOptionData = option.into();
    assert_eq!(data.get_as::<i64>("date_format"), Some(3)); // Friendly format
    assert_eq!(data.get_as::<i64>("time_format"), Some(0)); // TwelveHour format
    assert_eq!(data.get_as::<bool>("include_time"), Some(true));
    assert_eq!(
      data.get_as::<i64>("field_type"),
      Some(i64::from(FieldType::CreatedTime))
    );
  }

  #[test]
  fn test_formatted_date_time_from_timestamp() {
    let option = TimestampTypeOption {
      date_format: DateFormat::Friendly,
      time_format: TimeFormat::TwentyFourHour,
      include_time: true,
      field_type: FieldType::CreatedTime.into(),
      timezone: Some("Etc/UTC".to_string()),
    };

    let timestamp = Some(1672531200); // January 1, 2023 00:00:00 UTC
    let (date, time) = option.formatted_date_time_from_timestamp(&timestamp);

    assert_eq!(date, "Jan 01, 2023");
    assert_eq!(time, "00:00");
  }

  #[test]
  fn test_json_cell() {
    let option = TimestampTypeOption {
      date_format: DateFormat::US,
      time_format: TimeFormat::TwentyFourHour,
      include_time: true,
      field_type: FieldType::CreatedTime.into(),
      timezone: Some("Etc/UTC".to_string()),
    };

    let mut cell = Cell::new();
    cell.insert(CELL_DATA.into(), 1672531200.to_string().into()); // January 1, 2023 00:00:00 UTC

    let json_value = option.json_cell(&cell);
    assert_eq!(json_value, json!("2023-01-01T00:00:00+00:00"));
  }

  #[test]
  fn test_convert_raw_cell_data() {
    let option = TimestampTypeOption {
      date_format: DateFormat::ISO,
      time_format: TimeFormat::TwentyFourHour,
      include_time: false,
      field_type: FieldType::CreatedTime.into(),
      timezone: None,
    };

    let raw_data = "1672531200"; // January 1, 2023 00:00:00 UTC
    let result = option.convert_raw_cell_data(raw_data);

    assert_eq!(result, "2023-01-01");
  }

  #[test]
  fn timestamp_serde_to_cell() {
    let option = TimestampTypeOption::default();
    {
      let json_value = json!("1672531200");
      let cell = option.convert_json_to_cell(json_value);
      let ts_cell: TimestampCellData = (&cell).into();
      assert_eq!(ts_cell.timestamp, Some(1672531200));
    }
    {
      let json_value = json!(1672531200);
      let cell = option.convert_json_to_cell(json_value);
      let ts_cell: TimestampCellData = (&cell).into();
      assert_eq!(ts_cell.timestamp, Some(1672531200));
    }
  }

  #[test]
  fn timestamp_cell_to_serde() {
    let option = TimestampTypeOption::default();
    let ts_cell_data = TimestampCellData {
      timestamp: Some(1672531200),
    };
    let cell: Cell = ts_cell_data.to_cell(FieldType::CreatedTime);
    let json_value = option.json_cell(&cell);
    assert_eq!(json_value, "2023-01-01T00:00:00+00:00");
  }
}
