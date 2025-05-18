use crate::entity::FieldType;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RichTextTypeOption;

impl TypeOptionCellReader for RichTextTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    json!(self.stringify_cell(cell))
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    self.stringify_cell(cell).parse().ok()
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    text.to_string()
  }
}

impl TypeOptionCellWriter for RichTextTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let mut cell = new_cell_builder(FieldType::RichText);
    match json_value {
      Value::String(value_str) => {
        cell.insert(CELL_DATA.into(), value_str.into());
      },
      _ => {
        cell.insert(CELL_DATA.into(), json_value.to_string().into());
      },
    }
    cell
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

#[cfg(test)]
mod tests {
  use super::*;
  use collab::util::AnyMapExt;
  use serde_json::Value;

  #[test]
  fn rich_text_cell_to_serde() {
    let rich_text_type_option = RichTextTypeOption {};
    let cell_writer: Box<dyn TypeOptionCellReader> = Box::new(rich_text_type_option);
    {
      let mut cell: Cell = new_cell_builder(FieldType::RichText);
      cell.insert(CELL_DATA.into(), "helloworld".into());
      let serde_val = cell_writer.json_cell(&cell);
      assert_eq!(serde_val, Value::String("helloworld".to_string()));
    }
  }

  #[test]
  fn rich_text_serde_to_cell() {
    let rich_text_type_option = RichTextTypeOption {};
    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(rich_text_type_option);
    {
      // js string
      let cell: Cell = cell_writer.convert_json_to_cell(Value::String("helloworld".to_string()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "helloworld");
    }
    {
      // js number
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Number(10.into()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "10");
    }
  }
}
