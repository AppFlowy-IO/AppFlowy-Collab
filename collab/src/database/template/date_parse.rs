#![allow(deprecated)]
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};

pub fn cast_string_to_timestamp(cell: &str) -> Option<i64> {
  // Try to parse as a UNIX timestamp directly
  if let Ok(unix_timestamp) = cell.parse::<i64>() {
    // Only consider timestamps larger than a reasonable threshold (e.g., 1000000000 for the year 2001)
    if unix_timestamp > 1000000000 {
      return Utc
        .timestamp_opt(unix_timestamp, 0)
        .single()
        .filter(|&value| value.timestamp() > 0)
        .map(|value| value.timestamp());
    }
  }

  // Try to parse as datetime with time formats

  // Year-Month-Day Hour:Minute (24-hour format)
  if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(cell, "%Y-%m-%d %H:%M") {
    return Some(Utc.from_utc_datetime(&naive_datetime).timestamp());
  }

  // Year-Month-Day Hour:Minute AM/PM (12-hour format)
  if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(cell, "%Y-%m-%d %I:%M %p") {
    return Some(Utc.from_utc_datetime(&naive_datetime).timestamp());
  }

  // Try different date formats without time

  // Year-Month-Day
  if let Ok(naive_date) = NaiveDate::parse_from_str(cell, "%Y-%m-%d") {
    let datetime = naive_date.and_hms(0, 0, 0);
    return Some(Utc.from_utc_datetime(&datetime).timestamp());
  }

  // Year/Month/Day
  if let Ok(naive_date) = NaiveDate::parse_from_str(cell, "%Y/%m/%d") {
    let datetime = naive_date.and_hms(0, 0, 0);
    return Some(Utc.from_utc_datetime(&datetime).timestamp());
  }

  // Month/Day/Year
  if let Ok(naive_date) = NaiveDate::parse_from_str(cell, "%m/%d/%Y") {
    let datetime = naive_date.and_hms(0, 0, 0);
    return Some(Utc.from_utc_datetime(&datetime).timestamp());
  }

  // Month Day, Year
  if let Ok(naive_date) = NaiveDate::parse_from_str(cell, "%B %d, %Y") {
    let datetime = naive_date.and_hms(0, 0, 0);
    return Some(Utc.from_utc_datetime(&datetime).timestamp());
  }

  // Day/Month/Year
  if let Ok(naive_date) = NaiveDate::parse_from_str(cell, "%d/%m/%Y") {
    let datetime = naive_date.and_hms(0, 0, 0);
    return Some(Utc.from_utc_datetime(&datetime).timestamp());
  }

  None
}

pub(crate) fn replace_cells_with_timestamp(cells: Vec<String>) -> Vec<String> {
  cells
    .into_iter()
    .map(|cell| {
      cast_string_to_timestamp(&cell)
        .map_or_else(|| "".to_string(), |timestamp| timestamp.to_string())
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
  }
}
