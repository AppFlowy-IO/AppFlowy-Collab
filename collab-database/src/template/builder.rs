use crate::database::{gen_field_id, gen_row_id};
use crate::template::entity::{
  CELL_DATA, CellTemplateData, DatabaseTemplate, DatabaseViewTemplate, FieldTemplate, RowTemplate,
};

use crate::entity::FieldType;
use crate::fields::checkbox_type_option::CheckboxTypeOption;
use crate::fields::date_type_option::{DateFormat, DateTypeOption};
use crate::fields::media_type_option::MediaTypeOption;
use crate::fields::number_type_option::NumberTypeOption;
use crate::fields::select_type_option::SelectTypeOption;
use crate::fields::text_type_option::RichTextTypeOption;
use crate::fields::timestamp_type_option::TimestampTypeOption;
use crate::rows::new_cell_builder;
use crate::template::check_list_parse::ChecklistCellData;
use crate::template::csv::CSVResource;
use crate::template::date_parse::replace_cells_with_timestamp;
use crate::template::media_parse::replace_cells_with_files;
use crate::template::option_parse::{
  SELECT_OPTION_SEPARATOR, build_options_from_cells, replace_cells_with_options_id,
};
use crate::views::DatabaseLayout;

use collab::preclude::Any;

use std::collections::HashMap;

use std::path::Path;

#[async_trait::async_trait]
pub trait FileUrlBuilder: Send + Sync + 'static {
  async fn build(&self, database_id: &str, path: &Path) -> Option<String>;
}

pub struct DatabaseTemplateBuilder {
  #[allow(dead_code)]
  database_id: String,
  view_id: String,
  columns: Vec<Vec<CellTemplateData>>,
  fields: Vec<FieldTemplate>,
  file_url_builder: Option<Box<dyn FileUrlBuilder>>,
}

impl DatabaseTemplateBuilder {
  pub fn new(
    database_id: String,
    view_id: String,
    file_url_builder: Option<Box<dyn FileUrlBuilder>>,
  ) -> Self {
    Self {
      database_id,
      view_id,
      columns: vec![],
      fields: vec![],
      file_url_builder,
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn create_field<F>(
    mut self,
    csv_resource: &Option<CSVResource>,
    database_id: &str,
    name: &str,
    field_type: FieldType,
    is_primary: bool,
    field_builder: F,
  ) -> Self
  where
    F: FnOnce(FieldTemplateBuilder) -> FieldTemplateBuilder,
  {
    let builder = FieldTemplateBuilder::new(name.to_string(), field_type, is_primary);
    let (field, rows) = field_builder(builder)
      .build(csv_resource, database_id, &self.file_url_builder)
      .await;
    self.fields.push(field);
    self.columns.push(rows);
    self
  }

  pub fn build(self) -> DatabaseTemplate {
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
      database_id: self.database_id,
      view_id: self.view_id,
      fields,
      rows,
      views,
    }
  }
}

pub struct FieldTemplateBuilder {
  pub field_id: String,
  pub name: String,
  pub field_type: FieldType,
  pub is_primary: bool,
  cells: Vec<String>,
}

impl FieldTemplateBuilder {
  pub fn new(name: String, field_type: FieldType, is_primary: bool) -> Self {
    let field_id = gen_field_id();
    Self {
      field_id,
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

  pub async fn build(
    self,
    csv_resource: &Option<CSVResource>,
    database_id: &str,
    file_url_builder: &Option<Box<dyn FileUrlBuilder>>,
  ) -> (FieldTemplate, Vec<CellTemplateData>) {
    let field_type = self.field_type;
    let mut field_template = FieldTemplate {
      field_id: self.field_id,
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
              let mut map = new_cell_builder(field_type);
              map.insert(CELL_DATA.to_string(), Any::from(id));
              map
            })
            .collect::<Vec<CellTemplateData>>();

        field_template
          .type_options
          .insert(field_type, type_option.into());
        cell_template
      },
      FieldType::DateTime => {
        let cell_template = replace_cells_with_timestamp(self.cells)
          .into_iter()
          .map(|id| {
            let mut map = new_cell_builder(field_type);
            map.insert(CELL_DATA.to_string(), Any::from(id));
            map
          })
          .collect::<Vec<CellTemplateData>>();

        let mut type_option = DateTypeOption::new();
        type_option.date_format = DateFormat::FriendlyFull;

        field_template
          .type_options
          .insert(field_type, type_option.into());
        cell_template
      },
      FieldType::LastEditedTime | FieldType::CreatedTime => {
        let cell_template = replace_cells_with_timestamp(self.cells)
          .into_iter()
          .map(|id| {
            let mut map = new_cell_builder(field_type);
            map.insert(CELL_DATA.to_string(), Any::from(id));
            map
          })
          .collect::<Vec<CellTemplateData>>();
        let type_option = TimestampTypeOption::new(field_type);
        field_template
          .type_options
          .insert(field_type, type_option.into());
        cell_template
      },
      FieldType::RichText => {
        let cell_template = string_cell_template(&field_type, self.cells);
        field_template
          .type_options
          .insert(field_type, RichTextTypeOption.into());
        cell_template
      },
      FieldType::Checkbox => {
        let cell_template = string_cell_template(&field_type, self.cells);
        field_template
          .type_options
          .insert(field_type, CheckboxTypeOption.into());
        cell_template
      },
      FieldType::Number => {
        let cell_template = string_cell_template(&field_type, self.cells);
        field_template
          .type_options
          .insert(field_type, NumberTypeOption::default().into());

        cell_template
      },
      FieldType::Media => {
        let cell_template =
          replace_cells_with_files(self.cells, database_id, csv_resource, file_url_builder)
            .await
            .into_iter()
            .map(|file| {
              let mut cells = new_cell_builder(field_type);
              if let Some(file) = file {
                cells.insert(CELL_DATA.to_string(), Any::from(file));
              }
              cells
            })
            .collect();

        field_template
          .type_options
          .insert(field_type, MediaTypeOption::default().into());

        cell_template
      },
      _ => string_cell_template(&field_type, self.cells),
    };

    (field_template, cell_template)
  }
}

fn string_cell_template(field_type: &FieldType, cell: Vec<String>) -> Vec<CellTemplateData> {
  cell
    .into_iter()
    .map(|data| {
      let mut cells = new_cell_builder(field_type);
      cells.insert(CELL_DATA.to_string(), Any::from(data));
      cells
    })
    .collect()
}
