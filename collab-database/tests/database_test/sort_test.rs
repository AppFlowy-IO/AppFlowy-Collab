// use chrono::{FixedOffset, NaiveDateTime};
//
// #[test]
// fn utc_to_native() {
//   use chrono::Local;
//   let timestamp = 1647251762;
//   let native = NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap();
//
//   let offset = Local::now().offset().clone();
//   let auto_timezone = chrono::DateTime::<chrono::Local>::from_utc(native, offset);
//   println!("{}", auto_timezone);
//
//   let utc_7_offset = FixedOffset::east_opt(7 * 3600).unwrap();
//   let utc_7_timezone = chrono::DateTime::<chrono::Local>::from_utc(native, utc_7_offset);
//   println!("{}", utc_7_timezone);
// }
