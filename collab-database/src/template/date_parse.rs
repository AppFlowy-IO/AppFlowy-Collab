use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
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

pub(crate) fn replace_cells_with_timestamp(cells: Vec<String>) -> Vec<String> {
  cells
    .into_iter()
    .map(|cell| {
      // Try to parse as UNIX timestamp (i64)
      if let Ok(unix_timestamp) = cell.parse::<i64>() {
        return Utc
          .timestamp_opt(unix_timestamp, 0)
          .single()
          .map_or("".to_string(), |dt| dt.timestamp().to_string());
      }

      // Try to parse as datetime with time formats

      // Year-Month-Day Hour:Minute (24-hour format)
      if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(&cell, "%Y-%m-%d %H:%M") {
        return Utc
          .from_utc_datetime(&naive_datetime)
          .timestamp()
          .to_string();
      }

      // Year-Month-Day Hour:Minute AM/PM (12-hour format)
      if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(&cell, "%Y-%m-%d %I:%M %p") {
        return Utc
          .from_utc_datetime(&naive_datetime)
          .timestamp()
          .to_string();
      }

      // Try different date formats

      // Month/Day/Year
      if let Ok(naive_date) = NaiveDate::parse_from_str(&cell, "%m/%d/%Y") {
        let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Utc.from_utc_datetime(&datetime).timestamp().to_string();
      }

      // Year/Month/Day
      if let Ok(naive_date) = NaiveDate::parse_from_str(&cell, "%Y/%m/%d") {
        let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Utc.from_utc_datetime(&datetime).timestamp().to_string();
      }

      // Year-Month-Day
      if let Ok(naive_date) = NaiveDate::parse_from_str(&cell, "%Y-%m-%d") {
        let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Utc.from_utc_datetime(&datetime).timestamp().to_string();
      }

      // Month Day, Year
      if let Ok(naive_date) = NaiveDate::parse_from_str(&cell, "%B %d, %Y") {
        let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Utc.from_utc_datetime(&datetime).timestamp().to_string();
      }

      // Day/Month/Year
      if let Ok(naive_date) = NaiveDate::parse_from_str(&cell, "%d/%m/%Y") {
        let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Utc.from_utc_datetime(&datetime).timestamp().to_string();
      }

      // If no match, return an empty string
      "".to_string()
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use chrono::Utc;

  #[test]
  fn test_unix_timestamp_input() {
    // Input as UNIX timestamp should stay as is
    let cells = vec!["1726948800".to_string()];
    let result = replace_cells_with_timestamp(cells);
    assert_eq!(result[0], "1726948800");
  }

  #[test]
  fn test_month_day_year_format() {
    let cells = vec!["08/22/2024".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T00:00:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_year_month_day_format() {
    let cells = vec!["2024/08/22".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T00:00:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_year_month_day_hyphen_format() {
    let cells = vec!["2024-08-22".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T00:00:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_month_day_year_full_format() {
    let cells = vec!["August 22, 2024".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T00:00:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_day_month_year_format() {
    let cells = vec!["22/08/2024".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T00:00:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_24_hour_format() {
    let cells = vec!["2024-08-22 15:30".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T15:30:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(15, 30, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_12_hour_format() {
    let cells = vec!["2024-08-22 03:30 PM".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Expected Unix timestamp for "2024-08-22T15:30:00+00:00"
    assert_eq!(
      result[0],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(15, 30, 0)
        .timestamp()
        .to_string()
    );
  }

  #[test]
  fn test_invalid_format() {
    let cells = vec!["not-a-date".to_string()];
    let result = replace_cells_with_timestamp(cells);
    // Invalid input should return empty string
    assert_eq!(result[0], "");
  }

  #[test]
  fn test_mixed_inputs() {
    let cells = vec![
      "1726948800".to_string(),          // UNIX timestamp
      "2024-08-22".to_string(),          // ISO date
      "08/22/2024".to_string(),          // Month/Day/Year
      "2024-08-22 03:30 PM".to_string(), // 12-hour time
      "not-a-date".to_string(),          // Invalid input
    ];
    let result = replace_cells_with_timestamp(cells);
    assert_eq!(result[0], "1726948800");
    assert_eq!(
      result[1],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
    assert_eq!(
      result[2],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(0, 0, 0)
        .timestamp()
        .to_string()
    );
    assert_eq!(
      result[3],
      Utc
        .ymd(2024, 8, 22)
        .and_hms(15, 30, 0)
        .timestamp()
        .to_string()
    );
    assert_eq!(result[4], "");
  }
}
