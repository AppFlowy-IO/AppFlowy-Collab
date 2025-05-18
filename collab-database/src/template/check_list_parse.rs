use crate::database::gen_option_id;
use crate::entity::FieldType;
use crate::fields::select_type_option::{SelectOption, SelectOptionColor};
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;
use crate::template::util::{ToCellString, TypeOptionCellData};
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ChecklistCellData {
  pub options: Vec<SelectOption>,
  #[serde(default)]
  pub selected_option_ids: Vec<String>,
}

impl ChecklistCellData {
  pub fn selected_options(&self) -> Vec<SelectOption> {
    self
      .options
      .iter()
      .filter(|option| self.selected_option_ids.contains(&option.id))
      .cloned()
      .collect()
  }

  pub fn percentage_complete(&self) -> f64 {
    let selected_options = self.selected_option_ids.len();
    let total_options = self.options.len();

    if total_options == 0 {
      return 0.0;
    }
    ((selected_options as f64) / (total_options as f64) * 100.0).round() / 100.0
  }
}

impl From<&Cell> for ChecklistCellData {
  fn from(cell: &Cell) -> Self {
    cell
      .get_as::<String>(CELL_DATA)
      .map(|data| serde_json::from_str::<ChecklistCellData>(&data).unwrap_or_default())
      .unwrap_or_default()
  }
}

impl From<ChecklistCellData> for Cell {
  fn from(cell_data: ChecklistCellData) -> Self {
    let data = serde_json::to_string(&cell_data).unwrap_or_default();
    let mut cell = new_cell_builder(FieldType::Checklist);
    cell.insert(CELL_DATA.into(), data.into());
    cell
  }
}

impl TypeOptionCellData for ChecklistCellData {
  fn is_cell_empty(&self) -> bool {
    self.options.is_empty()
  }
}

impl From<(Vec<String>, Vec<String>)> for ChecklistCellData {
  fn from((names, selected_names): (Vec<String>, Vec<String>)) -> Self {
    let options: Vec<SelectOption> = names
      .into_iter()
      .enumerate()
      .map(|(index, name)| SelectOption {
        id: gen_option_id(),
        name: name.clone(),
        color: SelectOptionColor::from(index % 8),
      })
      .collect();

    let selected_option_ids: Vec<String> = selected_names
      .into_iter()
      .map(|name| {
        options
          .iter()
          .find(|opt| opt.name == name)
          .map_or_else(gen_option_id, |opt| opt.id.clone())
      })
      .collect();

    ChecklistCellData {
      options,
      selected_option_ids,
    }
  }
}

impl ToCellString for ChecklistCellData {
  fn to_cell_string(&self) -> String {
    serde_json::to_string(self).unwrap_or_default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::rows::Cell;

  #[test]
  fn test_checklist_cell_data_from_names_and_selected() {
    let names = vec![
      "Option 1".to_string(),
      "Option 2".to_string(),
      "Option 3".to_string(),
    ];
    let selected_names = vec!["Option 1".to_string(), "Option 3".to_string()];
    let checklist_data = ChecklistCellData::from((names, selected_names));

    assert_eq!(checklist_data.options.len(), 3);
    assert_eq!(checklist_data.selected_option_ids.len(), 2);

    let selected_names_set: Vec<_> = checklist_data
      .selected_option_ids
      .iter()
      .filter_map(|id| {
        checklist_data
          .options
          .iter()
          .find(|opt| opt.id == *id)
          .map(|opt| &opt.name)
      })
      .collect();

    assert_eq!(selected_names_set, vec!["Option 1", "Option 3"]);
  }

  #[test]
  fn test_checklist_cell_data_to_and_from_cell() {
    let names = vec!["Option A".to_string(), "Option B".to_string()];
    let selected_names = vec!["Option A".to_string()];
    let checklist_data = ChecklistCellData::from((names.clone(), selected_names.clone()));

    let cell: Cell = Cell::from(checklist_data.clone());
    let restored_data = ChecklistCellData::from(&cell);

    assert_eq!(restored_data.options.len(), checklist_data.options.len());
    assert_eq!(
      restored_data.selected_option_ids,
      checklist_data.selected_option_ids
    );
  }
}
