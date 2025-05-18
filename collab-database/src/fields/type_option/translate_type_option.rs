use super::{TypeOptionData, TypeOptionDataBuilder};
use crate::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::rows::Cell;
use crate::template::translate_parse::TranslateCellData;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use yrs::{Any, encoding::serde::from_any};

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

impl TypeOptionCellReader for TranslateTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    json!(self.stringify_cell(cell))
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    let cell = serde_json::from_str::<TranslateCellData>(cell_data).unwrap_or_default();
    cell.to_string()
  }
}

impl TypeOptionCellWriter for TranslateTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let cell = TranslateCellData(json_value.as_str().unwrap_or_default().to_string());
    cell.into()
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
