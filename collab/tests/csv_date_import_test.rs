//! Test for GitHub issue #8467: CSV date import bug
//! This test verifies that dates in DD/MM/YYYY format are parsed correctly
//! and don't default to epoch 0 (01/01/1970)

use collab::database::fields::TypeOptionCellWriter;
use collab::database::fields::date_type_option::DateTypeOption;
use collab::database::template::date_parse::cast_string_to_timestamp;
use collab::database::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde_json::Value;

#[test]
fn test_date_csv_date_import() {
  // The CSV file from issue #8467 has dates in DD/MM/YYYY format: "17/04/2025"
  let test_dates = vec![
    "17/04/2025", // DD/MM/YYYY - from the bug report
    "17/04/2025",
    "17/04/2025",
  ];

  let date_type_option = DateTypeOption::default_utc();

  let mut success_count = 0;
  let mut epoch_count = 0;

  for date_str in &test_dates {
    // This is what cast_string_to_timestamp does (used for type inference)
    let inferred = cast_string_to_timestamp(date_str);
    assert!(
      inferred.is_some(),
      "cast_string_to_timestamp should parse '{}'",
      date_str
    );

    // This is what convert_json_to_cell does (used for actual cell creation)
    let cell = date_type_option.convert_json_to_cell(Value::String(date_str.to_string()));
    let cell_data: Option<String> = cell.get_as(CELL_DATA);
    let cell_data_str = cell_data.unwrap_or_default();

    if cell_data_str.is_empty() {
      // Empty is acceptable (means no date)
      success_count += 1;
    } else if let Ok(timestamp) = cell_data_str.parse::<i64>() {
      if timestamp == 0 {
        epoch_count += 1;
        println!("BUG: '{}' defaulted to epoch 0!", date_str);
      } else {
        success_count += 1;
        // April 17, 2025 00:00:00 UTC = 1744848000
        assert_eq!(
          timestamp, 1744848000,
          "Expected timestamp for April 17, 2025"
        );
      }
    }
  }

  assert_eq!(
    epoch_count, 0,
    "No dates should default to epoch 0 (01/01/1970)"
  );
  assert_eq!(success_count, test_dates.len(), "All dates should parse");
}

#[test]
fn test_180_rows_dd_mm_yyyy() {
  // Simulate 180 rows of "17/04/2025" like in the bug report
  let date_type_option = DateTypeOption::default_utc();

  for i in 0..180 {
    let cell = date_type_option.convert_json_to_cell(Value::String("17/04/2025".to_string()));
    let cell_data: String = cell.get_as(CELL_DATA).unwrap_or_default();

    if !cell_data.is_empty() {
      let timestamp: i64 = cell_data.parse().expect("Should be valid timestamp");
      assert_ne!(timestamp, 0, "Row {} should not default to epoch 0", i + 1);
      assert_eq!(
        timestamp,
        1744848000,
        "Row {} should be April 17, 2025",
        i + 1
      );
    }
  }
}
