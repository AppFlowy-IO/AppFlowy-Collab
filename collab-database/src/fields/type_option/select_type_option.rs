use crate::database::gen_option_id;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  pub disable_color: bool,
}
impl SelectTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
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
