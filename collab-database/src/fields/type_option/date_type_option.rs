use crate::entity::FieldType;
use crate::error::DatabaseError;

use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use chrono::{FixedOffset, Local, MappedLocalTime, NaiveDateTime, NaiveTime, Offset, TimeZone};
use chrono_tz::Tz;
use collab::util::AnyMapExt;
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::template::time_parse::TimeCellData;
use crate::template::util::TypeOptionCellData;
use serde_json::{json, Value};
use std::str::FromStr;
pub use strum::IntoEnumIterator;
pub use strum_macros::EnumIter;
use tracing::error;
use yrs::Any;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TimeTypeOption;
impl TypeOptionCellReader for TimeTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = TimeCellData::from(cell);
    json!(cell_data)
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    let cell_data = TimeCellData::from(cell);
    cell_data.0.map(|timestamp| timestamp as f64)
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    let cell_data = TimeCellData::from(text);
    cell_data.to_string()
  }
}

impl TypeOptionCellWriter for TimeTypeOption {
  fn write_json(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<TimeCellData>(json_value).unwrap_or_default();
    Cell::from(&cell_data)
  }
}

impl From<TypeOptionData> for TimeTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    Self
  }
}

impl From<TimeTypeOption> for TypeOptionData {
  fn from(_data: TimeTypeOption) -> Self {
    TypeOptionDataBuilder::new()
  }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DateTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub timezone_id: String,
}

impl TypeOptionCellReader for DateTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = DateCellData::from(cell);
    json!(cell_data)
  }

  fn stringify_cell(&self, cell_data: &Cell) -> String {
    let cell_data = DateCellData::from(cell_data);
    let include_time = cell_data.include_time;
    let timestamp = cell_data.timestamp;
    let is_range = cell_data.is_range;

    let (date, time) = self.formatted_date_time_from_timestamp(&timestamp);
    if is_range {
      let (end_date, end_time) = match cell_data.end_timestamp {
        Some(timestamp) => self.formatted_date_time_from_timestamp(&Some(timestamp)),
        None => (date.clone(), time.clone()),
      };
      if include_time && timestamp.is_some() {
        format!("{} {} → {} {}", date, time, end_date, end_time)
          .trim()
          .to_string()
      } else if timestamp.is_some() {
        format!("{} → {}", date, end_date).trim().to_string()
      } else {
        "".to_string()
      }
    } else if include_time {
      format!("{} {}", date, time).trim().to_string()
    } else {
      date
    }
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    let cell_data = DateCellData::from(cell);
    cell_data.timestamp.map(|timestamp| timestamp as f64)
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    match text.parse::<i64>() {
      Ok(timestamp) => {
        let cell = DateCellData::from_timestamp(timestamp);
        Self::stringify_cell(self, &Cell::from(&cell))
      },
      Err(_) => "".to_string(),
    }
  }
}

impl TypeOptionCellWriter for DateTypeOption {
  fn write_json(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<DateCellData>(json_value).unwrap();
    Cell::from(&cell_data)
  }
}

impl DateTypeOption {
  pub fn new() -> Self {
    Self {
      date_format: DateFormat::default(),
      time_format: TimeFormat::default(),
      timezone_id: String::new(),
    }
  }

  pub fn default_utc() -> Self {
    Self {
      date_format: DateFormat::default(),
      time_format: TimeFormat::default(),
      timezone_id: "Etc/UTC".to_owned(),
    }
  }

  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }

  pub fn formatted_date_time_from_timestamp(&self, timestamp: &Option<i64>) -> (String, String) {
    if let Some(timestamp) = timestamp {
      match chrono::DateTime::from_timestamp(*timestamp, 0) {
        None => ("".to_owned(), "".to_owned()),
        Some(date) => {
          let naive = date.naive_utc();
          let offset = self.get_timezone_offset(naive);
          let date_time = chrono::DateTime::<Local>::from_naive_utc_and_offset(naive, offset);

          let fmt = self.date_format.format_str();
          let date = format!("{}", date_time.format(fmt));
          let fmt = self.time_format.format_str();
          let time = format!("{}", date_time.format(fmt));
          (date, time)
        },
      }
    } else {
      ("".to_owned(), "".to_owned())
    }
  }

  pub fn naive_time_from_time_string(
    &self,
    include_time: bool,
    time_str: Option<String>,
  ) -> Result<Option<NaiveTime>, DatabaseError> {
    match (include_time, time_str) {
      (true, Some(time_str)) => {
        let result = NaiveTime::parse_from_str(&time_str, self.time_format.format_str());
        match result {
          Ok(time) => Ok(Some(time)),
          Err(_e) => {
            let msg = format!("Parse {} failed", time_str);
            error!("{}", msg);
            Ok(None)
          },
        }
      },
      _ => Ok(None),
    }
  }

  /// combine the changeset_timestamp and parsed_time if provided. if
  /// changeset_timestamp is None, fallback to previous_timestamp
  pub fn timestamp_from_parsed_time_previous_and_new_timestamp(
    &self,
    parsed_time: Option<NaiveTime>,
    previous_timestamp: Option<i64>,
    changeset_timestamp: Option<i64>,
  ) -> Option<i64> {
    if let Some(time) = parsed_time {
      // a valid time is provided, so we replace the time component of old timestamp
      // (or new timestamp if provided) with it.
      let offset = changeset_timestamp
        .or(previous_timestamp)
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| self.get_timezone_offset(date.naive_utc()))?;

      let local_date = changeset_timestamp
        .or(previous_timestamp)
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| offset.from_utc_datetime(&date.naive_utc()).date_naive())?;

      match offset
        .from_local_datetime(&NaiveDateTime::new(local_date, time))
        .map(|date| date.timestamp())
      {
        MappedLocalTime::Single(value) => Some(value),
        MappedLocalTime::Ambiguous(_, _) => None,
        MappedLocalTime::None => None,
      }
    } else {
      changeset_timestamp.or(previous_timestamp)
    }
  }

  /// returns offset of Tz timezone if provided or of the local timezone otherwise
  fn get_timezone_offset(&self, date_time: NaiveDateTime) -> FixedOffset {
    let current_timezone_offset = Local::now().offset().fix();
    if self.timezone_id.is_empty() {
      current_timezone_offset
    } else {
      match Tz::from_str(&self.timezone_id) {
        Ok(timezone) => timezone.offset_from_utc_datetime(&date_time).fix(),
        Err(_) => current_timezone_offset,
      }
    }
  }
}

impl From<TypeOptionData> for DateTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let date_format = data
      .get_as::<i64>("date_format")
      .map(DateFormat::from)
      .unwrap_or_default();
    let time_format = data
      .get_as::<i64>("time_format")
      .map(TimeFormat::from)
      .unwrap_or_default();
    let timezone_id: String = data.get_as("timezone_id").unwrap_or_default();
    Self {
      date_format,
      time_format,
      timezone_id,
    }
  }
}

impl From<DateTypeOption> for TypeOptionData {
  fn from(data: DateTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("date_format".into(), Any::BigInt(data.date_format.value())),
      ("time_format".into(), Any::BigInt(data.time_format.value())),
      ("timezone_id".into(), data.timezone_id.into()),
    ])
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize, Default, EnumIter)]
pub enum TimeFormat {
  TwelveHour = 0,
  #[default]
  TwentyFourHour = 1,
}

impl std::convert::From<i64> for TimeFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => TimeFormat::TwelveHour,
      1 => TimeFormat::TwentyFourHour,
      _ => {
        tracing::error!("Unsupported time format, fallback to TwentyFourHour");
        TimeFormat::TwentyFourHour
      },
    }
  }
}
impl TimeFormat {
  pub fn value(&self) -> i64 {
    *self as i64
  }

  // https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html
  pub fn format_str(&self) -> &'static str {
    match self {
      TimeFormat::TwelveHour => "%I:%M %p",
      TimeFormat::TwentyFourHour => "%R",
    }
  }
}

#[derive(Clone, Debug, Copy, EnumIter, Serialize, Deserialize, Default, Eq, PartialEq)]
pub enum DateFormat {
  Local = 0,
  US = 1,
  ISO = 2,
  #[default]
  Friendly = 3,
  DayMonthYear = 4,
  FriendlyFull = 5,
}

impl std::convert::From<i64> for DateFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => DateFormat::Local,
      1 => DateFormat::US,
      2 => DateFormat::ISO,
      3 => DateFormat::Friendly,
      4 => DateFormat::DayMonthYear,
      5 => DateFormat::FriendlyFull,
      _ => {
        tracing::error!("Unsupported date format, fallback to friendly");
        DateFormat::Friendly
      },
    }
  }
}

impl DateFormat {
  pub fn value(&self) -> i64 {
    *self as i64
  }
  // https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html
  pub fn format_str(&self) -> &'static str {
    match self {
      DateFormat::Local => "%m/%d/%Y",
      DateFormat::US => "%Y/%m/%d",
      DateFormat::ISO => "%Y-%m-%d",
      DateFormat::Friendly => "%b %d, %Y",
      DateFormat::DayMonthYear => "%d/%m/%Y",
      DateFormat::FriendlyFull => "%B %-d, %Y",
    }
  }
}

#[derive(Default, Clone, Debug, Serialize)]
pub struct DateCellData {
  pub timestamp: Option<i64>,
  pub end_timestamp: Option<i64>,
  #[serde(default)]
  pub include_time: bool,
  #[serde(default)]
  pub is_range: bool,
  pub reminder_id: String,
}
impl TypeOptionCellData for DateCellData {
  fn is_empty(&self) -> bool {
    self.timestamp.is_none()
  }
}

impl DateCellData {
  pub fn new(timestamp: i64, include_time: bool, is_range: bool, reminder_id: String) -> Self {
    Self {
      timestamp: Some(timestamp),
      end_timestamp: None,
      include_time,
      is_range,
      reminder_id,
    }
  }

  pub fn from_timestamp(timestamp: i64) -> Self {
    Self {
      timestamp: Some(timestamp),
      end_timestamp: None,
      include_time: false,
      is_range: false,
      reminder_id: String::new(),
    }
  }
}

impl From<&Cell> for DateCellData {
  fn from(cell: &Cell) -> Self {
    let timestamp = cell
      .get_as::<String>(CELL_DATA)
      .and_then(|data| data.parse::<i64>().ok());
    let end_timestamp = cell
      .get_as::<String>("end_timestamp")
      .and_then(|data| data.parse::<i64>().ok());
    let include_time: bool = cell.get_as("include_time").unwrap_or_default();
    let is_range: bool = cell.get_as("is_range").unwrap_or_default();
    let reminder_id: String = cell.get_as("reminder_id").unwrap_or_default();

    Self {
      timestamp,
      end_timestamp,
      include_time,
      is_range,
      reminder_id,
    }
  }
}

impl From<&DateCellData> for Cell {
  fn from(cell_data: &DateCellData) -> Self {
    let timestamp_string = match cell_data.timestamp {
      Some(timestamp) => timestamp.to_string(),
      None => "".to_owned(),
    };
    let end_timestamp_string = match cell_data.end_timestamp {
      Some(timestamp) => timestamp.to_string(),
      None => "".to_owned(),
    };
    // Most of the case, don't use these keys in other places. Otherwise, we should define
    // constants for them.
    let mut cell = new_cell_builder(FieldType::DateTime);
    cell.insert(CELL_DATA.into(), timestamp_string.into());
    cell.insert("end_timestamp".into(), end_timestamp_string.into());
    cell.insert("include_time".into(), cell_data.include_time.into());
    cell.insert("is_range".into(), cell_data.is_range.into());
    cell.insert(
      "reminder_id".into(),
      cell_data.reminder_id.to_owned().into(),
    );
    cell
  }
}
impl<'de> serde::Deserialize<'de> for DateCellData {
  fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct DateCellVisitor;

    impl<'de> Visitor<'de> for DateCellVisitor {
      type Value = DateCellData;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON object representing DateCellData or an integer timestamp")
      }

      fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(DateCellData {
          timestamp: Some(value),
          end_timestamp: None,
          include_time: false,
          is_range: false,
          reminder_id: String::new(),
        })
      }

      fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        self.visit_i64(value as i64)
      }

      fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
      where
        M: serde::de::MapAccess<'de>,
      {
        let mut timestamp: Option<i64> = None;
        let mut end_timestamp: Option<i64> = None;
        let mut include_time: Option<bool> = None;
        let mut is_range: Option<bool> = None;
        let mut reminder_id: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
          match key.as_str() {
            "timestamp" => {
              timestamp = parse_optional_number(&mut map)?;
            },
            "end_timestamp" => {
              end_timestamp = parse_optional_number(&mut map)?;
            },
            "include_time" => {
              include_time = map.next_value().ok();
            },
            "is_range" => {
              is_range = map.next_value().ok();
            },
            "reminder_id" => {
              reminder_id = map.next_value().ok();
            },
            _ => {
              let _: serde_json::Value = map.next_value()?; // Ignore unknown keys
            },
          }
        }

        Ok(DateCellData {
          timestamp,
          end_timestamp,
          include_time: include_time.unwrap_or_default(),
          is_range: is_range.unwrap_or_default(),
          reminder_id: reminder_id.unwrap_or_default(),
        })
      }
    }

    deserializer.deserialize_any(DateCellVisitor)
  }
}

fn parse_optional_number<'de, M>(map: &mut M) -> core::result::Result<Option<i64>, M::Error>
where
  M: serde::de::MapAccess<'de>,
{
  match map.next_value::<serde_json::Value>() {
    Ok(serde_json::Value::Number(num)) => {
      if let Some(int) = num.as_i64() {
        Ok(Some(int))
      } else {
        Ok(None)
      }
    },
    Ok(serde_json::Value::String(s)) => s.parse::<i64>().ok().map(Some).ok_or_else(|| {
      serde::de::Error::custom(format!(
        "Expected a numeric value or parsable string, got {}",
        s
      ))
    }),
    Ok(_) => Ok(None),
    Err(_) => Ok(None),
  }
}
impl ToString for DateCellData {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_date_cell_data_from_cell() {
    let mut cell = Cell::new();
    cell.insert(CELL_DATA.into(), "1672531200".to_string().into()); // Timestamp for 2023-01-01T00:00:00Z
    cell.insert("end_timestamp".into(), "1672617600".to_string().into()); // Timestamp for 2023-01-02T00:00:00Z
    cell.insert("include_time".into(), true.into());
    cell.insert("is_range".into(), true.into());
    cell.insert("reminder_id".into(), "reminder123".to_string().into());

    let date_cell_data = DateCellData::from(&cell);
    assert_eq!(date_cell_data.timestamp, Some(1672531200));
    assert_eq!(date_cell_data.end_timestamp, Some(1672617600));
    assert!(date_cell_data.include_time);
    assert!(date_cell_data.is_range);
    assert_eq!(date_cell_data.reminder_id, "reminder123");
  }

  #[test]
  fn test_date_cell_data_to_cell() {
    let date_cell_data = DateCellData {
      timestamp: Some(1672531200),
      end_timestamp: Some(1672617600),
      include_time: true,
      is_range: true,
      reminder_id: "reminder123".to_string(),
    };

    let cell = Cell::from(&date_cell_data);
    assert_eq!(
      cell.get_as::<String>(CELL_DATA),
      Some("1672531200".to_string())
    );
    assert_eq!(
      cell.get_as::<String>("end_timestamp"),
      Some("1672617600".to_string())
    );
    assert_eq!(cell.get_as::<bool>("include_time"), Some(true));
    assert_eq!(cell.get_as::<bool>("is_range"), Some(true));
    assert_eq!(
      cell.get_as::<String>("reminder_id"),
      Some("reminder123".to_string())
    );
  }

  #[test]
  fn test_date_type_option_json_cell() {
    let date_type_option = DateTypeOption::default_utc();
    let mut cell = Cell::new();
    cell.insert(CELL_DATA.into(), "1672531200".to_string().into());

    let json_value = date_type_option.json_cell(&cell);
    assert_eq!(
      json_value,
      json!({
          "timestamp": 1672531200,
          "end_timestamp": null,
          "include_time": false,
          "is_range": false,
          "reminder_id": ""
      })
    );
  }

  #[test]
  fn test_date_type_option_stringify_cell() {
    let date_type_option = DateTypeOption::default_utc();
    let mut cell = Cell::new();
    cell.insert(CELL_DATA.into(), "1672531200".to_string().into());
    cell.insert("include_time".into(), true.into());

    let result = date_type_option.stringify_cell(&cell);
    assert_eq!(result, "Jan 01, 2023 00:00");
  }

  #[test]
  fn test_date_type_option_numeric_cell() {
    let date_type_option = DateTypeOption::default_utc();
    let mut cell = Cell::new();
    cell.insert(CELL_DATA.into(), "1672531200".to_string().into());

    let result = date_type_option.numeric_cell(&cell);
    assert_eq!(result, Some(1672531200.0));
  }

  #[test]
  fn test_date_type_option_write_json() {
    let date_type_option = DateTypeOption::default_utc();
    let json_value = json!({
        "timestamp": 1672531200,
        "end_timestamp": 1672617600,
        "include_time": true,
        "is_range": true,
        "reminder_id": "reminder123"
    });

    let cell = date_type_option.write_json(json_value);
    assert_eq!(
      cell.get_as::<String>(CELL_DATA),
      Some("1672531200".to_string())
    );
    assert_eq!(
      cell.get_as::<String>("end_timestamp"),
      Some("1672617600".to_string())
    );
    assert_eq!(cell.get_as::<bool>("include_time"), Some(true));
    assert_eq!(cell.get_as::<bool>("is_range"), Some(true));
    assert_eq!(
      cell.get_as::<String>("reminder_id"),
      Some("reminder123".to_string())
    );
  }

  #[test]
  fn test_date_type_option_convert_raw_cell_data() {
    let date_type_option = DateTypeOption::default_utc();

    let raw_data = "1672531200";
    let result = date_type_option.convert_raw_cell_data(raw_data);
    assert_eq!(result, "Jan 01, 2023");

    let invalid_raw_data = "invalid";
    let result = date_type_option.convert_raw_cell_data(invalid_raw_data);
    assert_eq!(result, "");
  }
}
