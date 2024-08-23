use crate::database::gen_option_id;
use crate::template::option_parse::{SelectOption, SelectOptionColor};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ChecklistCellData {
  pub options: Vec<SelectOption>,
  pub selected_option_ids: Vec<String>,
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
