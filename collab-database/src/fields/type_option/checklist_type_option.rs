use super::{TypeOptionData, TypeOptionDataBuilder};

use crate::fields::select_type_option::SELECTION_IDS_SEPARATOR;
use crate::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::rows::Cell;
use crate::template::check_list_parse::ChecklistCellData;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChecklistTypeOption;

impl From<TypeOptionData> for ChecklistTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    Self
  }
}

impl From<ChecklistTypeOption> for TypeOptionData {
  fn from(_data: ChecklistTypeOption) -> Self {
    TypeOptionDataBuilder::default()
  }
}

impl TypeOptionCellReader for ChecklistTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = ChecklistCellData::from(cell);
    json!(cell_data)
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    let cell_data = serde_json::from_str::<ChecklistCellData>(cell_data).unwrap_or_default();
    cell_data
      .options
      .into_iter()
      .map(|option| option.name)
      .collect::<Vec<_>>()
      .join(SELECTION_IDS_SEPARATOR)
  }
}

impl TypeOptionCellWriter for ChecklistTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<ChecklistCellData>(json_value).unwrap_or_default();
    cell_data.into()
  }
}

#[cfg(test)]
mod checklist_type_option_tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_json_cell_conversion() {
    let checklist_option = ChecklistTypeOption;

    let cell_data = ChecklistCellData::from((
      vec!["Opt1".to_string(), "Opt2".to_string()],
      vec!["Opt1".to_string()],
    ));
    let cell: Cell = cell_data.clone().into();

    let json_value = checklist_option.json_cell(&cell);
    let restored_data: ChecklistCellData =
      serde_json::from_value(json_value).expect("Valid JSON value");

    assert_eq!(restored_data.options.len(), 2);
    assert_eq!(restored_data.selected_option_ids.len(), 1);
  }

  #[test]
  fn test_numeric_cell_conversion() {
    let checklist_option = ChecklistTypeOption;

    let cell_data = ChecklistCellData::from((
      vec!["Opt1".to_string(), "Opt2".to_string()],
      vec!["Opt1".to_string()],
    ));
    let cell: Cell = cell_data.clone().into();

    let numeric_value = checklist_option.numeric_cell(&cell);
    assert!(numeric_value.is_none());
  }

  #[test]
  fn test_raw_cell_data_conversion() {
    let checklist_option = ChecklistTypeOption;

    let cell_data = ChecklistCellData::from((
      vec!["OptA".to_string(), "OptB".to_string()],
      vec!["OptA".to_string()],
    ));
    let cell_data_json = serde_json::to_string(&cell_data).expect("Valid serialization");

    let converted_data = checklist_option.convert_raw_cell_data(&cell_data_json);
    assert_eq!(converted_data, "OptA,OptB");
  }

  #[test]
  fn test_write_json_to_cell() {
    let checklist_option = ChecklistTypeOption;

    let json_value = json!({
        "options": [
            { "id": "1", "name": "Option1", "color": "Pink" },
            { "id": "2", "name": "Option2", "color": "LightPink" }
        ],
        "selected_option_ids": ["1"]
    });

    let cell = checklist_option.convert_json_to_cell(json_value);
    let restored_data = ChecklistCellData::from(&cell);

    assert_eq!(restored_data.options.len(), 2);
    assert_eq!(restored_data.selected_option_ids.len(), 1);
  }
}
