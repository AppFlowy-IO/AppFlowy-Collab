use crate::database::gen_option_id;
use crate::fields::select_type_option::{SelectOption, SelectOptionColor};
use std::collections::HashSet;

pub(crate) const SELECT_OPTION_SEPARATOR: &str = ",";
pub(crate) fn replace_cells_with_options_id(
  cells: Vec<String>,
  options: &[SelectOption],
  separator: &str,
) -> Vec<String> {
  cells
    .into_iter()
    .map(|cell| {
      cell
        .split(separator)
        .map(|part| {
          options
            .iter()
            .find(|option| option.name == part.trim())
            .map_or(part.to_string(), |option| option.id.clone())
        })
        .collect::<Vec<String>>()
        .join(separator)
    })
    .collect()
}

pub fn build_options_from_cells(cells: &[String]) -> Vec<SelectOption> {
  let mut option_names = HashSet::new();
  for cell in cells {
    cell.split(SELECT_OPTION_SEPARATOR).for_each(|cell| {
      let trim_cell = cell.trim();
      if !trim_cell.is_empty() {
        option_names.insert(trim_cell.to_string());
      }
    });
  }

  let mut options = vec![];
  for (index, name) in option_names.into_iter().enumerate() {
    // pick a color by mod 8
    let color = SelectOptionColor::from(index % 8);
    let option = SelectOption {
      id: gen_option_id(),
      name,
      color,
    };
    options.push(option);
  }

  options
}
