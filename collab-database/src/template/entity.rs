use crate::database::{gen_database_id, gen_database_view_id, gen_field_id, gen_row_id, timestamp};
use crate::entity::{CreateDatabaseParams, CreateViewParams};
use crate::fields::Field;
use crate::rows::{CreateRowParams, RowId};
use crate::views::{DatabaseLayout, LayoutSettings};
use collab::preclude::Any;
use std::collections::HashMap;

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
  pub name: String,
  pub field_type: FieldType,
  pub is_primary: bool,
  pub type_options: HashMap<FieldType, HashMap<String, Any>>,
}
const DEFAULT_IS_PRIMARY_VALUE: fn() -> bool = || false;

pub type CellTemplate = HashMap<String, CellTemplateData>;
pub type CellTemplateData = HashMap<String, Any>;

#[derive(Debug, Clone)]
pub struct RowTemplate {
  pub row_id: String,
  pub height: i32,
  pub visibility: bool,
  pub cells: CellTemplate,
}

impl Default for RowTemplate {
  fn default() -> Self {
    Self {
      row_id: Default::default(),
      height: 60,
      visibility: true,
      cells: Default::default(),
    }
  }
}

pub fn create_database_from_template(template: DatabaseTemplate) -> CreateDatabaseParams {
  let database_id = template.database_id.clone();
  let inline_view_id = gen_database_view_id();
  let timestamp = timestamp();

  let mut fields = vec![];
  for template_field in template.fields {
    let mut field = Field::new(
      gen_field_id(),
      template_field.name,
      template_field.field_type as i64,
      template_field.is_primary,
    );
    for (field_type, type_options) in template_field.type_options {
      field = field.with_type_option_data(field_type.type_id(), type_options);
    }
    fields.push(field);
  }

  let mut rows = vec![];
  for row_template in template.rows {
    rows.push(CreateRowParams {
      id: RowId::from(row_template.row_id),
      database_id: database_id.clone(),
      cells: row_template.cells,
      height: row_template.height,
      visibility: row_template.visibility,
      row_position: Default::default(),
      created_at: timestamp,
      modified_at: timestamp,
    });
  }

  let mut views = vec![];
  for view_template in template.views {
    views.push(CreateViewParams {
      database_id: database_id.clone(),
      view_id: gen_database_view_id(),
      name: view_template.name,
      layout: view_template.layout,
      layout_settings: view_template.layout_settings,
      filters: view_template.filters,
      group_settings: view_template.group_settings,
      sorts: view_template.sorts,
      field_settings: Default::default(),
      created_at: timestamp,
      modified_at: timestamp,
      deps_fields: vec![],
      deps_field_setting: vec![],
    });
  }

  CreateDatabaseParams {
    database_id,
    inline_view_id,
    fields,
    rows,
    views,
  }
}

#[derive(Clone, Hash, Eq, PartialEq)]
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
