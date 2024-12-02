use serde::{Deserialize, Serialize};
use yrs::{encoding::serde::from_any, Any};

use super::{TypeOptionData, TypeOptionDataBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateTypeOption {
  #[serde(default)]
  pub auto_fill: bool,
  /// Use [TranslateTypeOption::language_from_type] to get the language name
  #[serde(default, rename = "language")]
  pub language_type: i64,
}

impl TranslateTypeOption {
  pub fn language_from_type(language_type: i64) -> &'static str {
    match language_type {
      0 => "Traditional Chinese",
      1 => "English",
      2 => "French",
      3 => "German",
      4 => "Hindi",
      5 => "Spanish",
      6 => "Portuguese",
      7 => "Standard Arabic",
      8 => "Simplified Chinese",
      _ => "English",
    }
  }
}

impl Default for TranslateTypeOption {
  fn default() -> Self {
    Self {
      auto_fill: false,
      language_type: 1,
    }
  }
}

impl From<TypeOptionData> for TranslateTypeOption {
  fn from(data: TypeOptionData) -> Self {
    from_any(&Any::from(data)).unwrap()
  }
}

impl From<TranslateTypeOption> for TypeOptionData {
  fn from(value: TranslateTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("auto_fill".into(), value.auto_fill.into()),
      ("language".into(), Any::BigInt(value.language_type)),
    ])
  }
}
