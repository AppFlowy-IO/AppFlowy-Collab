use crate::entity::FieldType;
use crate::fields::date_type_option::{DateFormat, TimeFormat};
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use chrono::{DateTime, Local, Offset};
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use yrs::Any;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimestampTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub include_time: bool,
  pub field_type: i64,
}

impl TypeOptionCellReader for TimestampTypeOption {
  /// Return formated date and time string for the cell
  fn json_cell(&self, cell: &Cell) -> Value {
    let s = self.stringify_cell(cell);
    json!(s)
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    let (date_string, time_string) =
      self.formatted_date_time_from_timestamp(&text.parse::<i64>().ok());
    if self.include_time {
      format!("{} {}", date_string, time_string)
    } else {
      date_string
    }
  }
}

impl TypeOptionCellWriter for TimestampTypeOption {
  fn write_json(&self, json_value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::Time);
    if let Some(data) = match json_value {
      Value::String(s) => s.parse::<i64>().ok(),
      Value::Number(n) => n.as_i64(),
      _ => None,
    } {
      cell.insert(CELL_DATA.into(), data.into());
    }
    cell
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
      let offset = Local::now().offset().fix();
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
    Self {
      date_format,
      time_format,
      include_time,
      field_type,
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
