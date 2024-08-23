use crate::database::{gen_database_id, gen_field_id, gen_row_id};
use crate::template::entity::{
  CellTemplateData, DatabaseTemplate, DatabaseViewTemplate, FieldTemplate, FieldType, RowTemplate,
  CELL_DATA, TYPE_OPTION_CONTENT,
};

use crate::template::chect_list_parse::ChecklistCellData;
use crate::template::date_parse::{replace_cells_with_timestamp, DateTypeOption};
use crate::template::option_parse::{
  build_options_from_cells, replace_cells_with_options_id, SelectTypeOption,
  SELECT_OPTION_SEPARATOR,
};
use crate::template::time_parse::TimestampTypeOption;
use crate::views::DatabaseLayout;
use collab::preclude::Any;
use std::collections::HashMap;

pub struct DatabaseTemplateBuilder {
  columns: Vec<Vec<CellTemplateData>>,
  fields: Vec<FieldTemplate>,
}

impl Default for DatabaseTemplateBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl DatabaseTemplateBuilder {
  pub fn new() -> Self {
    Self {
      columns: vec![],
      fields: vec![],
    }
  }

  pub fn create_field<F>(
    mut self,
    name: &str,
    field_type: FieldType,
    is_primary: bool,
    f: F,
  ) -> Self
  where
    F: FnOnce(FieldTemplateBuilder) -> FieldTemplateBuilder,
  {
    let builder = FieldTemplateBuilder::new(name.to_string(), field_type, is_primary);
    let (field, rows) = f(builder).build();
    self.fields.push(field);
    self.columns.push(rows);
    self
  }

  pub fn build(self) -> DatabaseTemplate {
    let database_id = gen_database_id();
    let fields = self.fields;

    let num_rows = self
      .columns
      .iter()
      .map(|column| column.len())
      .max()
      .unwrap_or(0);

    let mut rows = Vec::with_capacity(num_rows);
    for _ in 0..num_rows {
      rows.push(RowTemplate {
        row_id: gen_row_id().to_string(),
        height: 60,
        visibility: true,
        cells: Default::default(),
      });
    }

    for (field_index, row) in self.columns.into_iter().enumerate() {
      for (row_index, cell) in row.into_iter().enumerate() {
        rows[row_index]
          .cells
          .insert(fields[field_index].field_id.clone(), cell);
      }
    }

    let views = vec![DatabaseViewTemplate {
      name: "".to_string(),
      layout: DatabaseLayout::Grid,
      layout_settings: Default::default(),
      filters: vec![],
      group_settings: vec![],
      sorts: vec![],
    }];

    DatabaseTemplate {
      database_id,
      fields,
      rows,
      views,
    }
  }
}

pub struct FieldTemplateBuilder {
  pub name: String,
  pub field_type: FieldType,
  pub is_primary: bool,
  cells: Vec<String>,
}

impl FieldTemplateBuilder {
  pub fn new(name: String, field_type: FieldType, is_primary: bool) -> Self {
    Self {
      name,
      field_type,
      is_primary,
      cells: vec![],
    }
  }

  pub fn create_cell<T: ToString>(mut self, cell: T) -> Self {
    self.cells.push(cell.to_string());
    self
  }

  pub fn create_checklist_cell<T1: ToString, T2: ToString>(
    mut self,
    options: Vec<T1>,
    selected_options: Vec<T2>,
  ) -> Self {
    let options = options
      .into_iter()
      .map(|option| option.to_string())
      .collect();
    let selected_options = selected_options
      .into_iter()
      .map(|option| option.to_string())
      .collect();
    let cell = ChecklistCellData::from((options, selected_options));
    self
      .cells
      .push(serde_json::to_string(&cell).unwrap_or_default());
    self
  }

  pub fn build(self) -> (FieldTemplate, Vec<CellTemplateData>) {
    let field_type = self.field_type.clone();
    let mut field_template = FieldTemplate {
      field_id: gen_field_id(),
      name: self.name,
      field_type: self.field_type,
      is_primary: self.is_primary,
      type_options: HashMap::new(),
    };

    let cell_template = match field_type {
      FieldType::SingleSelect | FieldType::MultiSelect => {
        let options = build_options_from_cells(&self.cells);
        let type_option = SelectTypeOption {
          options,
          disable_color: false,
        };
        let cell_template =
          replace_cells_with_options_id(self.cells, &type_option.options, SELECT_OPTION_SEPARATOR)
            .into_iter()
            .map(|id| {
              let mut map: HashMap<String, Any> = HashMap::new();
              map.insert(CELL_DATA.to_string(), Any::from(id));
              map
            })
            .collect::<Vec<CellTemplateData>>();

        field_template.type_options.insert(
          field_type,
          HashMap::from([(
            TYPE_OPTION_CONTENT.to_string(),
            Any::from(type_option.to_json_string()),
          )]),
        );
        cell_template
      },
      FieldType::DateTime => {
        let cell_template = replace_cells_with_timestamp(self.cells)
          .into_iter()
          .map(|id| {
            let mut map: HashMap<String, Any> = HashMap::new();
            map.insert(CELL_DATA.to_string(), Any::from(id));
            map
          })
          .collect::<Vec<CellTemplateData>>();

        field_template.type_options.insert(
          field_type,
          HashMap::from([(
            TYPE_OPTION_CONTENT.to_string(),
            Any::from(DateTypeOption::default().to_json_string()),
          )]),
        );
        cell_template
      },
      FieldType::LastEditedTime | FieldType::CreatedTime => {
        let cell_template = replace_cells_with_timestamp(self.cells)
          .into_iter()
          .map(|id| {
            let mut map: HashMap<String, Any> = HashMap::new();
            map.insert(CELL_DATA.to_string(), Any::from(id));
            map
          })
          .collect::<Vec<CellTemplateData>>();

        let type_option =
          serde_json::to_string(&TimestampTypeOption::new(field_type.clone(), false)).unwrap();
        field_template.type_options.insert(
          field_type,
          HashMap::from([(TYPE_OPTION_CONTENT.to_string(), Any::from(type_option))]),
        );
        cell_template
      },
      _ => string_cell_template(self.cells),
    };

    (field_template, cell_template)
  }
}

fn string_cell_template(cell: Vec<String>) -> Vec<CellTemplateData> {
  cell
    .into_iter()
    .map(|data| HashMap::from([(CELL_DATA.to_string(), Any::from(data))]))
    .collect()
}
