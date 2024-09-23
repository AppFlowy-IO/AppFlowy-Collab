use crate::fields::{StringifyTypeOption, TypeOptionData, TypeOptionDataBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckboxTypeOption;

impl CheckboxTypeOption {
  pub fn new() -> Self {
    Self
  }
}

impl StringifyTypeOption for CheckboxTypeOption {
  fn stringify_text(&self, text: &str) -> String {
    text.to_string()
  }
}

impl From<CheckboxTypeOption> for TypeOptionData {
  fn from(_data: CheckboxTypeOption) -> Self {
    TypeOptionDataBuilder::new()
  }
}

impl From<TypeOptionData> for CheckboxTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    CheckboxTypeOption
  }
}
