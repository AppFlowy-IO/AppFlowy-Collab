use crate::database::gen_row_id;
use crate::views::{DatabaseLayout, LayoutSettings};
use collab::preclude::Any;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
pub const CELL_DATA: &str = "data";
pub const TYPE_OPTION_CONTENT: &str = "content";
pub struct DatabaseTemplate {
  pub database_id: String,
  pub fields: Vec<FieldTemplate>,
  pub rows: Vec<RowTemplate>,
  pub views: Vec<DatabaseViewTemplate>,
}
pub struct DatabaseViewTemplate {
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<HashMap<String, Any>>,
  pub group_settings: Vec<HashMap<String, Any>>,
  pub sorts: Vec<HashMap<String, Any>>,
}

pub struct FieldTemplate {
  pub field_id: String,
  pub name: String,
  pub field_type: FieldType,
  pub is_primary: bool,
  pub type_options: HashMap<FieldType, HashMap<String, Any>>,
}

pub type CellTemplate = HashMap<String, CellTemplateData>;
pub type CellTemplateData = HashMap<String, Any>;

#[derive(Debug, Clone)]
pub struct RowTemplate {
  pub row_id: String,
  pub height: i32,
  pub visibility: bool,
  pub cells: CellTemplate,
}

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
  RichText = 0,
  Number = 1,
  DateTime = 2,
  SingleSelect = 3,
  MultiSelect = 4,
  Checkbox = 5,
  URL = 6,
  Checklist = 7,
  LastEditedTime = 8,
  CreatedTime = 9,
  Relation = 10,
  Summary = 11,
  Translate = 12,
  Time = 13,
}

impl FieldType {
  pub fn type_id(&self) -> String {
    (self.clone() as i64).to_string()
  }
}
