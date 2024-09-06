use crate::entity::FieldType;
use crate::views::{DatabaseLayout, LayoutSettings};
use collab::preclude::Any;

use std::collections::HashMap;

pub const CELL_DATA: &str = "data";
pub const TYPE_OPTION_CONTENT: &str = "content";
pub struct DatabaseTemplate {
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
