use crate::database::entity::{CreateDatabaseParams, FieldType};
use crate::database::views::{DatabaseLayout, LayoutSettings};
use crate::entity::uuid_validation::{DatabaseId, DatabaseViewId};
use crate::preclude::Any;

use crate::database::error::DatabaseError;
use crate::database::template::util::create_database_params_from_template;
use std::collections::HashMap;

pub const CELL_DATA: &str = "data";
pub struct DatabaseTemplate {
  pub database_id: DatabaseId,
  pub view_id: DatabaseViewId,
  pub fields: Vec<FieldTemplate>,
  pub rows: Vec<RowTemplate>,
  pub views: Vec<DatabaseViewTemplate>,
}

impl DatabaseTemplate {
  pub fn into_params(self) -> Result<CreateDatabaseParams, DatabaseError> {
    create_database_params_from_template(self)
  }
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
