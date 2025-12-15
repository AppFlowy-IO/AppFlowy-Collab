use collab::database::entity::{FieldType, default_type_option_data_from_type};
use collab::database::fields::rollup_type_option::{RollupDisplayMode, RollupTypeOption};
use collab::database::fields::{TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData};
use collab::database::rows::Cell;
use collab::database::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde_json::json;
use yrs::Any;

// ==================== RollupDisplayMode Tests ====================

#[test]
fn rollup_display_mode_from_i64_calculated() {
  assert_eq!(RollupDisplayMode::from(0), RollupDisplayMode::Calculated);
}

#[test]
fn rollup_display_mode_from_i64_original_list() {
  assert_eq!(RollupDisplayMode::from(1), RollupDisplayMode::OriginalList);
}

#[test]
fn rollup_display_mode_from_i64_unique_list() {
  assert_eq!(RollupDisplayMode::from(2), RollupDisplayMode::UniqueList);
}

#[test]
fn rollup_display_mode_from_i64_unknown_defaults_to_calculated() {
  assert_eq!(RollupDisplayMode::from(99), RollupDisplayMode::Calculated);
  assert_eq!(RollupDisplayMode::from(-1), RollupDisplayMode::Calculated);
}

#[test]
fn rollup_display_mode_to_i64() {
  assert_eq!(i64::from(RollupDisplayMode::Calculated), 0);
  assert_eq!(i64::from(RollupDisplayMode::OriginalList), 1);
  assert_eq!(i64::from(RollupDisplayMode::UniqueList), 2);
}

#[test]
fn rollup_display_mode_default() {
  assert_eq!(RollupDisplayMode::default(), RollupDisplayMode::Calculated);
}

// ==================== RollupTypeOption Default Tests ====================

#[test]
fn rollup_type_option_default_values() {
  let option = RollupTypeOption::default();
  assert_eq!(option.relation_field_id, "");
  assert_eq!(option.target_field_id, "");
  assert_eq!(option.calculation_type, 5); // Default to Count
  assert_eq!(option.show_as, RollupDisplayMode::Calculated);
  assert_eq!(option.condition_value, "");
}

// ==================== RollupTypeOption Serialization Tests ====================

#[test]
fn rollup_type_option_to_type_option_data() {
  let option = RollupTypeOption {
    relation_field_id: "rel_field_1".to_string(),
    target_field_id: "target_field_1".to_string(),
    calculation_type: 3,
    show_as: RollupDisplayMode::OriginalList,
    condition_value: "some_condition".to_string(),
  };

  let data: TypeOptionData = option.into();

  assert_eq!(
    data.get_as::<String>("relation_field_id").unwrap(),
    "rel_field_1"
  );
  assert_eq!(
    data.get_as::<String>("target_field_id").unwrap(),
    "target_field_1"
  );
  assert_eq!(data.get_as::<i64>("calculation_type").unwrap(), 3);
  assert_eq!(data.get_as::<i64>("show_as").unwrap(), 1);
  assert_eq!(
    data.get_as::<String>("condition_value").unwrap(),
    "some_condition"
  );
}

#[test]
fn rollup_type_option_from_type_option_data() {
  let mut data = TypeOptionData::new();
  data.insert(
    "relation_field_id".to_string(),
    Any::String("rel_field_2".into()),
  );
  data.insert(
    "target_field_id".to_string(),
    Any::String("target_field_2".into()),
  );
  data.insert("calculation_type".to_string(), Any::BigInt(7));
  data.insert("show_as".to_string(), Any::BigInt(2));
  data.insert(
    "condition_value".to_string(),
    Any::String("condition_2".into()),
  );

  let option = RollupTypeOption::from(data);

  assert_eq!(option.relation_field_id, "rel_field_2");
  assert_eq!(option.target_field_id, "target_field_2");
  assert_eq!(option.calculation_type, 7);
  assert_eq!(option.show_as, RollupDisplayMode::UniqueList);
  assert_eq!(option.condition_value, "condition_2");
}

#[test]
fn rollup_type_option_from_empty_type_option_data() {
  let data = TypeOptionData::new();
  let option = RollupTypeOption::from(data);

  // Should use default values when data is missing
  assert_eq!(option.relation_field_id, "");
  assert_eq!(option.target_field_id, "");
  assert_eq!(option.calculation_type, 5); // Default to Count
  assert_eq!(option.show_as, RollupDisplayMode::Calculated);
  assert_eq!(option.condition_value, "");
}

#[test]
fn rollup_type_option_roundtrip() {
  let original = RollupTypeOption {
    relation_field_id: "rel_123".to_string(),
    target_field_id: "target_456".to_string(),
    calculation_type: 10,
    show_as: RollupDisplayMode::UniqueList,
    condition_value: "my_condition".to_string(),
  };

  let data: TypeOptionData = original.clone().into();
  let restored = RollupTypeOption::from(data);

  assert_eq!(restored.relation_field_id, original.relation_field_id);
  assert_eq!(restored.target_field_id, original.target_field_id);
  assert_eq!(restored.calculation_type, original.calculation_type);
  assert_eq!(restored.show_as, original.show_as);
  assert_eq!(restored.condition_value, original.condition_value);
}

// ==================== TypeOptionCellReader Tests ====================

#[test]
fn rollup_type_option_json_cell() {
  let option = RollupTypeOption::default();
  let mut cell = Cell::new();
  cell.insert(CELL_DATA.to_string(), Any::String("test_value".into()));

  let json = option.json_cell(&cell);
  assert_eq!(json, json!("test_value"));
}

#[test]
fn rollup_type_option_json_cell_empty() {
  let option = RollupTypeOption::default();
  let cell = Cell::new();

  let json = option.json_cell(&cell);
  assert_eq!(json, json!(""));
}

#[test]
fn rollup_type_option_numeric_cell_valid() {
  let option = RollupTypeOption::default();
  let mut cell = Cell::new();
  cell.insert(CELL_DATA.to_string(), Any::String("42.5".into()));

  let numeric = option.numeric_cell(&cell);
  assert_eq!(numeric, Some(42.5));
}

#[test]
fn rollup_type_option_numeric_cell_invalid() {
  let option = RollupTypeOption::default();
  let mut cell = Cell::new();
  cell.insert(CELL_DATA.to_string(), Any::String("not_a_number".into()));

  let numeric = option.numeric_cell(&cell);
  assert_eq!(numeric, None);
}

#[test]
fn rollup_type_option_convert_raw_cell_data() {
  let option = RollupTypeOption::default();
  let result = option.convert_raw_cell_data("raw_data_test");
  assert_eq!(result, "raw_data_test");
}

// ==================== TypeOptionCellWriter Tests ====================

#[test]
fn rollup_type_option_convert_json_string_to_cell() {
  let option = RollupTypeOption::default();
  let json_value = json!("hello world");

  let cell = option.convert_json_to_cell(json_value);

  assert_eq!(
    cell.get(CELL_DATA).unwrap(),
    &Any::String("hello world".into())
  );
}

#[test]
fn rollup_type_option_convert_json_number_to_cell() {
  let option = RollupTypeOption::default();
  let json_value = json!(123);

  let cell = option.convert_json_to_cell(json_value);

  assert_eq!(cell.get(CELL_DATA).unwrap(), &Any::String("123".into()));
}

#[test]
fn rollup_type_option_convert_json_object_to_cell() {
  let option = RollupTypeOption::default();
  let json_value = json!({"key": "value"});

  let cell = option.convert_json_to_cell(json_value);

  assert_eq!(
    cell.get(CELL_DATA).unwrap(),
    &Any::String("{\"key\":\"value\"}".into())
  );
}

#[test]
fn rollup_type_option_convert_json_array_to_cell() {
  let option = RollupTypeOption::default();
  let json_value = json!([1, 2, 3]);

  let cell = option.convert_json_to_cell(json_value);

  assert_eq!(
    cell.get(CELL_DATA).unwrap(),
    &Any::String("[1,2,3]".into())
  );
}

// ==================== FieldType::Rollup Tests ====================

#[test]
fn field_type_rollup_value() {
  assert_eq!(FieldType::Rollup as i64, 16);
}

#[test]
fn field_type_from_i64_rollup() {
  let field_type = FieldType::from(16_i64);
  assert_eq!(field_type, FieldType::Rollup);
}

#[test]
fn field_type_rollup_is_rollup() {
  assert!(FieldType::Rollup.is_rollup());
  assert!(!FieldType::RichText.is_rollup());
  assert!(!FieldType::Relation.is_rollup());
}

#[test]
fn field_type_rollup_default_name() {
  assert_eq!(FieldType::Rollup.default_name(), "Rollup");
}

#[test]
fn field_type_rollup_default_type_option_data() {
  let data = default_type_option_data_from_type(FieldType::Rollup);
  let option = RollupTypeOption::from(data);

  // Verify it creates a valid default RollupTypeOption
  assert_eq!(option.relation_field_id, "");
  assert_eq!(option.target_field_id, "");
  assert_eq!(option.calculation_type, 5);
  assert_eq!(option.show_as, RollupDisplayMode::Calculated);
  assert_eq!(option.condition_value, "");
}
