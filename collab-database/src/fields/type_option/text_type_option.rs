use crate::fields::{StringifyTypeOption, TypeOptionData, TypeOptionDataBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RichTextTypeOption;

impl StringifyTypeOption for RichTextTypeOption {
  fn stringify_text(&self, text: &str) -> String {
    text.to_string()
  }
}

impl From<TypeOptionData> for RichTextTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    RichTextTypeOption
  }
}

impl From<RichTextTypeOption> for TypeOptionData {
  fn from(_data: RichTextTypeOption) -> Self {
    TypeOptionDataBuilder::new()
  }
}
