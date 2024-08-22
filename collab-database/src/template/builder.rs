use crate::database::{gen_database_id, gen_option_id};
use crate::template::entity::{
  CellTemplate, CellTemplateData, DatabaseTemplate, DatabaseViewTemplate, FieldTemplate, FieldType,
  RowTemplate,
};

use serde::{Deserialize, Serialize};

use crate::views::DatabaseLayout;
use collab::preclude::Any;
use std::collections::{HashMap, HashSet};

pub struct DatabaseTemplateBuilder {
  columns: Vec<Vec<CellTemplateData>>,
  fields: Vec<FieldTemplate>,
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

    let mut rows = vec![RowTemplate::default(); num_rows];
    for (field_index, row) in self.columns.into_iter().enumerate() {
      for (row_index, cell) in row.into_iter().enumerate() {
        rows[row_index]
          .cells
          .insert(fields[field_index].field_type.type_id(), cell);
      }
    }

    let mut views = vec![];
    // create inline view
    views.push(DatabaseViewTemplate {
      name: "".to_string(),
      layout: DatabaseLayout::Grid,
      layout_settings: Default::default(),
      filters: vec![],
      group_settings: vec![],
      sorts: vec![],
    });

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

const CELL_DATA: &str = "data";
const TYPE_OPTION_CONTENT: &str = "content";
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

  pub fn build(self) -> (FieldTemplate, Vec<CellTemplateData>) {
    let field_type = self.field_type.clone();
    let mut field_template = FieldTemplate {
      name: self.name,
      field_type: self.field_type,
      is_primary: self.is_primary,
      type_options: HashMap::new(),
    };

    let cell_template = match field_type {
      FieldType::SingleSelect => {
        let options = build_options_from_cells(&self.cells);
        let type_option = SelectTypeOption {
          options,
          disable_color: false,
        };
        let cell_template = replace_cells_with_options_id(self.cells, &type_option.options)
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
      FieldType::MultiSelect => {
        todo!()
      },
      FieldType::Checklist => {
        todo!()
      },
      FieldType::Relation => {
        vec![]
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

fn replace_cells_with_options_id(cells: Vec<String>, options: &[SelectOption]) -> Vec<String> {
  cells
    .into_iter()
    .map(|cell| {
      let option = options.iter().find(|option| option.name == cell).unwrap();
      option.id.clone()
    })
    .collect()
}

fn build_options_from_cells(cells: &[String]) -> Vec<SelectOption> {
  let mut option_names = HashSet::new();
  for cell in cells {
    option_names.insert(cell.clone());
  }

  let mut options = vec![];
  for (index, name) in option_names.into_iter().enumerate() {
    // pick a color by mod 8
    let color = SelectOptionColor::from(index % 8);
    let option = SelectOption {
      id: gen_option_id(),
      name,
      color,
    };
    options.push(option);
  }

  options
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  pub disable_color: bool,
}
impl SelectTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SelectOption {
  pub id: String,
  pub name: String,
  pub color: SelectOptionColor,
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

#[derive(Debug)]
pub struct SelectOptionIds(Vec<String>);
impl SelectOptionIds {
  pub fn from_cell(cell: String) -> Self {
    let ids = cell
      .split(",")
      .map(|id| id.to_string())
      .collect::<Vec<String>>();
    Self(ids)
  }
}
