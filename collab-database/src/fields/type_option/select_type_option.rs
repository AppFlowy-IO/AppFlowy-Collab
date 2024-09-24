use crate::database::gen_option_id;

use crate::error::DatabaseError;
use crate::fields::{StringifyTypeOption, TypeOptionData, TypeOptionDataBuilder};
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  pub disable_color: bool,
}

impl StringifyTypeOption for SelectTypeOption {
  fn stringify_text(&self, text: &str) -> String {
    let ids = SelectOptionIds::from_str(text).unwrap_or_default().0;
    if ids.is_empty() {
      return "".to_string();
    }

    let options = ids
      .iter()
      .flat_map(|option_id| {
        self
          .options
          .iter()
          .find(|option| &option.id == option_id)
          .map(|option| option.name.clone())
      })
      .collect::<Vec<_>>();
    options.join(", ")
  }
}

impl SelectTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

impl From<TypeOptionData> for SelectTypeOption {
  fn from(data: TypeOptionData) -> Self {
    data
      .get_as::<String>("content")
      .map(|s| serde_json::from_str::<SelectTypeOption>(&s).unwrap_or_default())
      .unwrap_or_default()
  }
}

impl From<SelectTypeOption> for TypeOptionData {
  fn from(data: SelectTypeOption) -> Self {
    let content = serde_json::to_string(&data).unwrap_or_default();
    TypeOptionDataBuilder::from([("content".into(), content.into())])
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectOption {
  pub id: String,
  pub name: String,
  pub color: SelectOptionColor,
}
impl SelectOption {
  pub fn new(name: &str) -> Self {
    SelectOption {
      id: gen_option_id(),
      name: name.to_owned(),
      color: SelectOptionColor::default(),
    }
  }

  pub fn with_color(name: &str, color: SelectOptionColor) -> Self {
    SelectOption {
      id: gen_option_id(),
      name: name.to_owned(),
      color,
    }
  }
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

#[derive(Clone, Default, Debug)]
pub struct SingleSelectTypeOption(pub SelectTypeOption);

impl StringifyTypeOption for SingleSelectTypeOption {
  fn stringify_text(&self, text: &str) -> String {
    self.0.stringify_text(text)
  }
}

impl Deref for SingleSelectTypeOption {
  type Target = SelectTypeOption;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for SingleSelectTypeOption {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl From<TypeOptionData> for SingleSelectTypeOption {
  fn from(data: TypeOptionData) -> Self {
    SingleSelectTypeOption(SelectTypeOption::from(data))
  }
}

impl From<SingleSelectTypeOption> for TypeOptionData {
  fn from(data: SingleSelectTypeOption) -> Self {
    data.0.into()
  }
}

// Multiple select
#[derive(Clone, Default, Debug)]
pub struct MultiSelectTypeOption(pub SelectTypeOption);
impl StringifyTypeOption for MultiSelectTypeOption {
  fn stringify_text(&self, text: &str) -> String {
    self.0.stringify_text(text)
  }
}

impl Deref for MultiSelectTypeOption {
  type Target = SelectTypeOption;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for MultiSelectTypeOption {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl From<TypeOptionData> for MultiSelectTypeOption {
  fn from(data: TypeOptionData) -> Self {
    MultiSelectTypeOption(SelectTypeOption::from(data))
  }
}

impl From<MultiSelectTypeOption> for TypeOptionData {
  fn from(data: MultiSelectTypeOption) -> Self {
    data.0.into()
  }
}

#[derive(Default, Clone, Debug)]
pub struct SelectOptionIds(Vec<String>);
impl SelectOptionIds {
  pub fn new() -> Self {
    Self::default()
  }
  pub fn into_inner(self) -> Vec<String> {
    self.0
  }
  pub fn to_cell_data(&self, field_type: impl Into<i64>) -> Cell {
    let mut cell = new_cell_builder(field_type);
    cell.insert(CELL_DATA.into(), self.to_string().into());
    cell
  }
}

pub const SELECTION_IDS_SEPARATOR: &str = ",";

impl std::convert::From<Vec<String>> for SelectOptionIds {
  fn from(ids: Vec<String>) -> Self {
    let ids = ids
      .into_iter()
      .filter(|id| !id.is_empty())
      .collect::<Vec<String>>();
    Self(ids)
  }
}

impl ToString for SelectOptionIds {
  /// Returns a string that consists list of ids, placing a commas
  /// separator between each
  fn to_string(&self) -> String {
    self.0.join(SELECTION_IDS_SEPARATOR)
  }
}

impl From<&Cell> for SelectOptionIds {
  fn from(cell: &Cell) -> Self {
    let value: String = cell.get_as(CELL_DATA).unwrap_or_default();
    Self::from_str(&value).unwrap_or_default()
  }
}

impl FromStr for SelectOptionIds {
  type Err = DatabaseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.is_empty() {
      return Ok(Self(vec![]));
    }
    let ids = s
      .split(SELECTION_IDS_SEPARATOR)
      .map(|id| id.to_string())
      .collect::<Vec<String>>();
    Ok(Self(ids))
  }
}

impl std::ops::Deref for SelectOptionIds {
  type Target = Vec<String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::ops::DerefMut for SelectOptionIds {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
