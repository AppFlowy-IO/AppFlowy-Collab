use crate::database::gen_option_id;
use serde::{Deserialize, Serialize};
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

pub(crate) fn build_options_from_cells(cells: &[String]) -> Vec<SelectOption> {
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

#[derive(Clone, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  pub disable_color: bool,
}
impl SelectTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SelectOption {
  pub id: String,
  pub name: String,
  pub color: SelectOptionColor,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
#[repr(u8)]
#[derive(Default)]
pub enum SelectOptionColor {
  #[default]
  Purple = 0,
  Pink = 1,
  LightPink = 2,
  Orange = 3,
  Yellow = 4,
  Lime = 5,
  Green = 6,
  Aqua = 7,
  Blue = 8,
}

impl From<usize> for SelectOptionColor {
  fn from(index: usize) -> Self {
    match index {
      0 => SelectOptionColor::Purple,
      1 => SelectOptionColor::Pink,
      2 => SelectOptionColor::LightPink,
      3 => SelectOptionColor::Orange,
      4 => SelectOptionColor::Yellow,
      5 => SelectOptionColor::Lime,
      6 => SelectOptionColor::Green,
      7 => SelectOptionColor::Aqua,
      8 => SelectOptionColor::Blue,
      _ => SelectOptionColor::Purple,
    }
  }
}

#[derive(Debug)]
pub struct SelectOptionIds(Vec<String>);
impl SelectOptionIds {
  pub fn from_cell(cell: String) -> Self {
    let ids = cell
      .split(SELECT_OPTION_SEPARATOR)
      .map(|id| id.to_string())
      .collect::<Vec<String>>();
    Self(ids)
  }
}
