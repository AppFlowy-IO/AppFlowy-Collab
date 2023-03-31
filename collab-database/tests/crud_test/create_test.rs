use crate::helper::create_database;
use assert_json_diff::{assert_json_eq, assert_json_matches};

#[test]
fn create_initial_database_test() {
  let database_test = create_database("1");
  let json = database_test.to_json_value();

  assert_json_eq!("", json);
}
