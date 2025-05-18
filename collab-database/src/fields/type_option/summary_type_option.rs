use super::{TypeOptionData, TypeOptionDataBuilder};
use crate::fields::{TypeOptionCellReader, TypeOptionCellWriter};
use crate::rows::Cell;
use crate::template::summary_parse::SummaryCellData;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummarizationTypeOption {
  pub auto_fill: bool,
}

impl From<TypeOptionData> for SummarizationTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let auto_fill: bool = data.get_as("auto_fill").unwrap_or_default();
    Self { auto_fill }
  }
}

impl From<SummarizationTypeOption> for TypeOptionData {
  fn from(data: SummarizationTypeOption) -> Self {
    TypeOptionDataBuilder::from([("auto_fill".into(), data.auto_fill.into())])
  }
}

impl TypeOptionCellReader for SummarizationTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    let cell_data = SummaryCellData::from(cell);
    json!(cell_data)
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, cell_data: &str) -> String {
    let cell_data = SummaryCellData(cell_data.to_string());
    cell_data.to_string()
  }
}

impl TypeOptionCellWriter for SummarizationTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<SummaryCellData>(json_value).unwrap_or_default();
    cell_data.into()
  }
}
