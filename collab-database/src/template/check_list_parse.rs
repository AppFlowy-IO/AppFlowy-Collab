use crate::database::gen_option_id;
use crate::fields::select_type_option::{SelectOption, SelectOptionColor};
use crate::template::util::TypeOptionCellData;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ChecklistCellData {
  pub options: Vec<SelectOption>,
  #[serde(default)]
  pub selected_option_ids: Vec<String>,
}

impl TypeOptionCellData for ChecklistCellData {
  fn is_empty(&self) -> bool {
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
