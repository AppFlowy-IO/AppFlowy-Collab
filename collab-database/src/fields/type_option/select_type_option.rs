use crate::database::gen_option_id;

use crate::entity::FieldType;
use crate::error::DatabaseError;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{Cell, new_cell_builder};
use crate::template::entity::CELL_DATA;

use crate::template::util::{ToCellString, TypeOptionCellData};
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  #[serde(default)]
  pub disable_color: bool,
}

impl TypeOptionCellReader for SelectTypeOption {
  /// Returns list of selected options
  fn json_cell(&self, cell: &Cell) -> Value {
    match cell.get_as::<String>(CELL_DATA) {
      None => Value::Null,
      Some(s) => {
        let ids = SelectOptionIds::from_str(&s).unwrap_or_default().0;
        if ids.is_empty() {
          return Value::Array(vec![]);
        }

        let options = ids
          .iter()
          .flat_map(|option_id| {
            self
              .options
              .iter()
              .find(|option| &option.id == option_id)
              .cloned()
          })
          .collect::<Vec<_>>();
        json!(options)
      },
    }
  }

  fn numeric_cell(&self, _cell: &Cell) -> Option<f64> {
    None
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
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
  #[serde(default)]
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

impl TryFrom<u8> for SelectOptionColor {
  type Error = &'static str;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(SelectOptionColor::Purple),
      1 => Ok(SelectOptionColor::Pink),
      2 => Ok(SelectOptionColor::LightPink),
      3 => Ok(SelectOptionColor::Orange),
      4 => Ok(SelectOptionColor::Yellow),
      5 => Ok(SelectOptionColor::Lime),
      6 => Ok(SelectOptionColor::Green),
      7 => Ok(SelectOptionColor::Aqua),
      8 => Ok(SelectOptionColor::Blue),
      _ => Err("Invalid color value"),
    }
  }
}

impl From<SelectOptionColor> for u8 {
  fn from(color: SelectOptionColor) -> Self {
    color as u8
  }
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

impl TypeOptionCellReader for SingleSelectTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    match cell.get_as::<String>(CELL_DATA) {
      None => Value::Null,
      Some(id) => self
        .options
        .iter()
        .find(|option| option.id == id)
        .map(|option| option.name.clone())
        .unwrap_or_default()
        .into(),
    }
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    self.0.numeric_cell(cell)
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    self.0.convert_raw_cell_data(text)
  }
}

impl TypeOptionCellWriter for SingleSelectTypeOption {
  fn convert_json_to_cell(&self, value: Value) -> Cell {
    cell_from_json_value(value, &self.options, FieldType::SingleSelect)
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
impl TypeOptionCellReader for MultiSelectTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    match cell.get_as::<String>(CELL_DATA) {
      None => Value::Array(vec![]),
      Some(s) => {
        let ids = SelectOptionIds::from_str(&s).unwrap_or_default().0;
        if ids.is_empty() {
          return Value::Array(vec![]);
        }
        ids
          .iter()
          .flat_map(|option_id| {
            self
              .options
              .iter()
              .find(|option| &option.id == option_id)
              .cloned()
          })
          .map(|option| option.name.clone())
          .collect::<Vec<String>>()
          .into()
      },
    }
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    self.0.numeric_cell(cell)
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    self.0.convert_raw_cell_data(text)
  }
}

impl TypeOptionCellWriter for MultiSelectTypeOption {
  fn convert_json_to_cell(&self, value: Value) -> Cell {
    cell_from_json_value(value, &self.options, FieldType::MultiSelect)
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
  pub fn to_cell(&self, field_type: impl Into<i64>) -> Cell {
    let mut cell = new_cell_builder(field_type);
    cell.insert(CELL_DATA.into(), self.to_cell_string().into());
    cell
  }
}

impl TypeOptionCellData for SelectOptionIds {
  fn is_cell_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl ToCellString for SelectOptionIds {
  fn to_cell_string(&self) -> String {
    self.0.join(SELECTION_IDS_SEPARATOR)
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
fn cell_from_json_value(value: Value, options: &[SelectOption], field_type: FieldType) -> Cell {
  match value {
    Value::Array(array) => {
      // Process array of JSON objects or strings
      let ids = array
        .iter()
        .filter_map(|item| match item {
          Value::Object(obj) => obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
              obj.get("name").and_then(|v| v.as_str()).and_then(|name| {
                options
                  .iter()
                  .find(|opt| opt.name == name)
                  .map(|opt| opt.id.clone())
              })
            }),
          Value::String(name) => options
            .iter()
            .find(|opt| &opt.name == name)
            .map(|opt| opt.id.clone()),
          _ => None,
        })
        .collect::<Vec<_>>();

      SelectOptionIds::from(ids).to_cell(field_type)
    },

    Value::String(s) => {
      // Process a single string (comma-separated names or IDs)
      let ids = s
        .split(SELECTION_IDS_SEPARATOR)
        .map(str::trim)
        .filter_map(|name| {
          options
            .iter()
            .find(|opt| opt.name == name)
            .map(|opt| opt.id.clone())
        })
        .collect::<Vec<_>>();

      SelectOptionIds::from(ids).to_cell(field_type)
    },
    Value::Object(obj) => {
      // Process a single object with "id" or "name"
      if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
        if options.iter().any(|opt| opt.id == id) {
          return SelectOptionIds::from(vec![id.to_string()]).to_cell(field_type);
        }
      }

      if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        if let Some(option) = options.iter().find(|opt| opt.name == name) {
          return SelectOptionIds::from(vec![option.id.clone()]).to_cell(field_type);
        }
      }
      SelectOptionIds::new().to_cell(field_type)
    },
    _ => SelectOptionIds::new().to_cell(field_type),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{Value, json};

  #[test]
  fn test_serialize_deserialize_select_type_option() {
    let options = vec![
      SelectOption::new("Option 1"),
      SelectOption::with_color("Option 2", SelectOptionColor::Blue),
    ];

    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };

    let serialized = serde_json::to_string(&select_type_option).unwrap();
    let deserialized: SelectTypeOption = serde_json::from_str(&serialized).unwrap();

    assert_eq!(select_type_option.disable_color, deserialized.disable_color);
    assert_eq!(select_type_option.options.len(), deserialized.options.len());
    assert_eq!(select_type_option.options[0].name, "Option 1");
    assert_eq!(select_type_option.options[1].color, SelectOptionColor::Blue);
  }

  #[test]
  fn test_select_option_ids_to_string() {
    let ids = SelectOptionIds::from(vec!["id1".to_string(), "id2".to_string()]);
    assert_eq!(ids.to_cell_string(), "id1,id2");
  }

  #[test]
  fn test_select_option_ids_from_str() {
    let ids = SelectOptionIds::from_str("id1,id2").unwrap();
    assert_eq!(ids.0, vec!["id1".to_string(), "id2".to_string()]);
  }

  #[test]
  fn test_cell_from_json_value_array() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];

    let value = json!([
        { "id": options[0].id },
        { "name": "Option 2" }
    ]);

    let cell = cell_from_json_value(value, &options, FieldType::MultiSelect);
    let cell_data: String = cell.get_as(CELL_DATA).unwrap();
    assert!(cell_data.contains(&options[0].id));
    assert!(cell_data.contains(&options[1].id));
  }

  #[test]
  fn test_cell_from_json_value_string() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];

    let value = Value::String("Option 1,Option 2".to_string());
    let cell = cell_from_json_value(value, &options, FieldType::MultiSelect);

    let cell_data: String = cell.get_as(CELL_DATA).unwrap();
    assert!(cell_data.contains(&options[0].id));
    assert!(cell_data.contains(&options[1].id));
  }

  #[test]
  fn test_single_select_type_option_write_json() {
    let options = vec![SelectOption::new("Option A"), SelectOption::new("Option B")];
    let single_select = SingleSelectTypeOption(SelectTypeOption {
      options,
      disable_color: false,
    });

    let json_value = json!({ "name": "Option A" });
    let cell = single_select.convert_json_to_cell(json_value);

    let cell_data: String = cell.get_as(CELL_DATA).unwrap();
    assert!(!cell_data.is_empty());
  }

  #[test]
  fn test_multi_select_type_option_write_json() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let multi_select = MultiSelectTypeOption(SelectTypeOption {
      options,
      disable_color: false,
    });

    let json_value = json!([
        { "name": "Option 1" },
        { "id": multi_select.options[1].id }
    ]);

    let cell = multi_select.convert_json_to_cell(json_value);
    let cell_data: String = cell.get_as(CELL_DATA).unwrap();
    assert!(cell_data.contains(&multi_select.options[0].id));
    assert!(cell_data.contains(&multi_select.options[1].id));
  }

  #[test]
  fn test_select_option_with_color() {
    let option = SelectOption::with_color("Colored Option", SelectOptionColor::Aqua);
    assert_eq!(option.name, "Colored Option");
    assert_eq!(option.color, SelectOptionColor::Aqua);
  }

  #[test]
  fn test_select_option_color_from_u8() {
    assert_eq!(
      SelectOptionColor::try_from(0_u8).unwrap(),
      SelectOptionColor::Purple
    );
    assert_eq!(
      SelectOptionColor::try_from(8_u8).unwrap(),
      SelectOptionColor::Blue
    );
    assert!(SelectOptionColor::try_from(10_u8).is_err());
  }

  #[test]
  fn test_convert_raw_cell_data() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let raw_data = options
      .iter()
      .map(|option| option.id.clone())
      .collect::<Vec<_>>()
      .join(",");

    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };

    let result = select_type_option.convert_raw_cell_data(&raw_data);
    assert_eq!(result, "Option 1, Option 2");
  }

  #[test]
  fn test_select_content_deser() {
    let js_str = r#"{
      "options": [
        {
          "id": "CEZD",
          "name": "To Do",
          "color": "Purple"
        },
        {
          "id": "TznH",
          "name": "Doing",
          "color": "Orange"
        },
        {
          "id": "__n6",
          "name": "✅ Done",
          "color": "Yellow"
        }
      ],
      "disable_color": false
    }"#;

    let select_ty_opt = serde_json::from_str::<SelectTypeOption>(js_str).unwrap();
    assert_eq!(select_ty_opt.options.len(), 3);
    assert_eq!(select_ty_opt.options[0].name, "To Do");
    assert_eq!(select_ty_opt.options[1].color, SelectOptionColor::Orange);
    assert_eq!(select_ty_opt.options[2].id, "__n6");
    assert!(!select_ty_opt.disable_color);
  }

  #[test]
  fn single_select_cell_to_serde() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let option_1_id = options[0].id.clone();
    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };
    let single_select = SingleSelectTypeOption(select_type_option);
    let single_select_cell_reader: Box<dyn TypeOptionCellReader> = Box::new(single_select);
    let mut cell: Cell = new_cell_builder(FieldType::SingleSelect);
    cell.insert(CELL_DATA.into(), option_1_id.into());
    let serde_val = single_select_cell_reader.json_cell(&cell);
    assert_eq!(serde_val, Value::String("Option 1".to_string()));
  }

  #[test]
  fn multi_select_cell_to_serde() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let option_1_id = options[0].id.clone();
    let option_2_id = options[1].id.clone();
    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };

    let multi_selection_type_option = MultiSelectTypeOption(select_type_option);
    let multi_select_cell_reader: Box<dyn TypeOptionCellReader> =
      Box::new(multi_selection_type_option);
    {
      // single select
      let mut cell: Cell = new_cell_builder(FieldType::MultiSelect);
      cell.insert(CELL_DATA.into(), option_1_id.clone().into());
      let serde_val = multi_select_cell_reader.json_cell(&cell);
      assert_eq!(
        serde_val,
        Value::Array(vec![Value::String("Option 1".to_string())])
      );
    }
    {
      // double select
      let mut cell: Cell = new_cell_builder(FieldType::MultiSelect);
      cell.insert(CELL_DATA.into(), (option_1_id + "," + &option_2_id).into());
      let serde_val = multi_select_cell_reader.json_cell(&cell);
      assert_eq!(
        serde_val,
        Value::Array(vec![
          Value::String("Option 1".to_string()),
          Value::String("Option 2".to_string())
        ])
      );
    }
    {
      // no select
      let cell: Cell = new_cell_builder(FieldType::MultiSelect);
      let serde_val = multi_select_cell_reader.json_cell(&cell);
      assert_eq!(serde_val, Value::Array(vec![]));
    }
  }

  #[test]
  fn single_select_serde_to_cell() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let option_1_id = options[0].id.clone();
    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };
    let single_select = SingleSelectTypeOption(select_type_option);

    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(single_select);
    {
      let cell: Cell = cell_writer.convert_json_to_cell(Value::String("Option 1".to_string()));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, option_1_id);
    }
  }

  #[test]
  fn multi_select_serde_to_cell() {
    let options = vec![SelectOption::new("Option 1"), SelectOption::new("Option 2")];
    let option_1_id = options[0].id.clone();
    let option_2_id = options[1].id.clone();
    let select_type_option = SelectTypeOption {
      options,
      disable_color: false,
    };
    let single_select = SingleSelectTypeOption(select_type_option);

    let cell_writer: Box<dyn TypeOptionCellWriter> = Box::new(single_select);
    {
      // No select
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Array(vec![]));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, "");
    }
    {
      // 1 select
      let cell: Cell =
        cell_writer.convert_json_to_cell(Value::Array(vec![Value::String("Option 1".to_string())]));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, option_1_id);
    }
    {
      // 2 select
      let cell: Cell = cell_writer.convert_json_to_cell(Value::Array(vec![
        Value::String("Option 1".to_string()),
        Value::String("Option 2".to_string()),
      ]));
      let data = cell.get_as::<String>(CELL_DATA).unwrap();
      assert_eq!(data, option_1_id + "," + &option_2_id);
    }
  }
}
