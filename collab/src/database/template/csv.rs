use crate::database::database::{gen_database_id, gen_database_view_id};
use crate::database::entity::FieldType;
use crate::database::template::builder::{DatabaseTemplateBuilder, FileUrlBuilder};
use crate::database::template::date_parse::cast_string_to_timestamp;
use crate::database::template::entity::DatabaseTemplate;
use crate::entity::uuid_validation::{DatabaseId, DatabaseViewId};
use crate::error::CollabError;
use percent_encoding::percent_decode_str;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use std::io;

pub struct CSVTemplate {
  pub fields: Vec<CSVField>,
  pub rows: Vec<Vec<String>>,
  pub resource: Option<CSVResource>,
  pub database_id: DatabaseId,
  pub view_id: DatabaseViewId,
}

pub struct CSVField {
  name: String,
  field_type: FieldType,
}

pub struct CSVResource {
  pub server_url: String,
  pub workspace_id: String,
  pub files: Vec<String>,
}

impl CSVTemplate {
  pub fn try_from_reader(
    reader: impl io::Read,
    auto_field_type: bool,
    mut csv_resource: Option<CSVResource>,
  ) -> Result<Self, CollabError> {
    let mut fields: Vec<CSVField> = vec![];

    let mut reader = csv::Reader::from_reader(reader);
    if let Ok(headers) = reader.headers() {
      for header in headers {
        fields.push(CSVField {
          name: header.to_string(),
          field_type: FieldType::RichText,
        });
      }
    } else {
      return Err(CollabError::DatabaseInvalidCsv("No header".to_string()));
    }

    let rows: Vec<Vec<String>> = reader
      .records()
      .flat_map(|r| r.ok())
      .map(|record| {
        record
          .into_iter()
          .filter_map(|s| Some(percent_decode_str(s).decode_utf8().ok()?.to_string()))
          .collect::<Vec<String>>()
      })
      .collect();

    if auto_field_type {
      auto_detect_field_type(&mut fields, &rows, &csv_resource);
    }

    // filter out resources that are not used
    filter_out_resources(&fields, &rows, &mut csv_resource);

    Ok(CSVTemplate {
      fields,
      rows,
      resource: csv_resource,
      database_id: gen_database_id(),
      view_id: gen_database_view_id(),
    })
  }

  pub fn reset_view_id(&mut self, view_id: DatabaseViewId) {
    self.view_id = view_id;
  }

  pub async fn try_into_database_template(
    self,
    file_url_builder: Option<Box<dyn FileUrlBuilder>>,
  ) -> Result<DatabaseTemplate, CollabError> {
    let CSVTemplate {
      fields,
      rows,
      resource,
      database_id,
      view_id,
    } = self;

    let mut builder = DatabaseTemplateBuilder::new(database_id, view_id, file_url_builder);
    for (field_index, field) in fields.into_iter().enumerate() {
      builder = builder
        .create_field(
          &resource,
          &database_id,
          &field.name,
          field.field_type,
          field_index == 0,
          |mut field_builder| {
            for row in rows.iter() {
              if let Some(cell) = row.get(field_index) {
                field_builder = field_builder.create_cell(cell)
              }
            }
            field_builder
          },
        )
        .await;
    }

    Ok(builder.build())
  }
}

fn filter_out_resources(
  fields: &[CSVField],
  rows: &[Vec<String>],
  resource: &mut Option<CSVResource>,
) {
  let mut cell_resources = HashSet::new();
  for (index, field) in fields.iter().enumerate() {
    if matches!(field.field_type, FieldType::Media) {
      for row in rows.iter() {
        if let Some(cell) = row.get(index) {
          for res in cell.split(',') {
            cell_resources.insert(res.to_string());
          }
        }
      }
    }
  }

  if let Some(resource) = resource {
    resource.files.retain(|file| {
      // retain if resource end with one of the cell resources
      cell_resources
        .iter()
        .any(|cell_res| file.ends_with(cell_res))
    });
  }
}

fn auto_detect_field_type(
  fields: &mut Vec<CSVField>,
  rows: &[Vec<String>],
  resources: &Option<CSVResource>,
) {
  let num_fields = fields.len();
  fields
    .par_iter_mut()
    .enumerate()
    .for_each(|(field_index, field)| {
      let cells: Vec<&str> = rows
        .par_iter()
        .filter_map(|row| {
          if row.len() != num_fields {
            None
          } else {
            Some(row[field_index].as_str())
          }
        })
        .collect();

      field.field_type = detect_field_type_from_cells_with_resource(&cells, resources);
    });
}

#[allow(dead_code)]
fn detect_field_type_from_cells(cells: &[&str]) -> FieldType {
  detect_field_type_from_cells_with_resource(cells, &None)
}

fn detect_field_type_from_cells_with_resource(
  cells: &[&str],
  resources: &Option<CSVResource>,
) -> FieldType {
  let cells = cells
    .iter()
    .filter(|cell| !cell.is_empty())
    .take(10)
    .cloned()
    .collect::<Vec<&str>>();

  if is_number_cell(&cells) {
    return FieldType::Number;
  }

  // Do not chang the order of the following checks
  if is_media_cell(&cells, resources) {
    return FieldType::Media;
  }

  if is_link_field(&cells) {
    return FieldType::URL;
  }

  if is_checkbox_cell(&cells) {
    return FieldType::Checkbox;
  }

  if is_date_cell(&cells) {
    // TODO(nathan): handle this case: April 23, 2024 → May 22, 2024
    return FieldType::DateTime;
  }

  if is_single_select_field(&cells) {
    return FieldType::SingleSelect;
  }

  if is_multi_select_field(&cells) {
    return FieldType::MultiSelect;
  }

  FieldType::RichText
}

fn is_media_cell(cells: &[&str], resource: &Option<CSVResource>) -> bool {
  let half_count = cells.len() / 2;
  let valid_count = cells
    .iter()
    .filter(|cell| match resource {
      Some(resource) => resource.files.iter().any(|file| file.ends_with(*cell)),
      None => false,
    })
    .count();

  if valid_count == 0 {
    return false;
  }

  valid_count >= half_count
}

fn is_date_cell(cells: &[&str]) -> bool {
  let half_count = cells.len() / 2;
  let valid_count = cells
    .iter()
    .filter(|&&cell| cast_string_to_timestamp(cell).is_some())
    .count();

  if valid_count == 0 {
    return false;
  }

  valid_count >= half_count
}

/// Detect if a column is a checkbox field.
/// Optimized by checking if valid checkbox values (e.g., "Yes", "No", "1", "0", "True", "False")
/// appear in multiple cells and returning early if a non-checkbox value is found.
fn is_checkbox_cell(cells: &[&str]) -> bool {
  // Define the set of valid checkbox values (in lowercase for case-insensitive comparison)
  let valid_checkbox_values: HashSet<&str> = ["yes", "no", "1", "0", "true", "false"]
    .iter()
    .cloned()
    .collect();

  // Track how many valid checkbox values we encounter
  let mut valid_checkbox_count = 0;

  // Early exit strategy: Iterate through the cells and check their values
  for &cell in cells {
    // Convert the cell value to lowercase and trim any whitespace
    let trimmed_cell = cell.trim().to_lowercase();

    // Check if the cell contains a valid checkbox value
    if valid_checkbox_values.contains(trimmed_cell.as_str()) {
      valid_checkbox_count += 1;
    } else {
      // If a cell has an invalid value, return false early
      return false;
    }
  }

  // Determine if the field is checkbox-like:
  // We require a minimum threshold (e.g., 50% of cells should be valid checkbox values).
  let total_cells = cells.len();
  let threshold = total_cells / 2;

  // Return true if the valid checkbox count exceeds the threshold
  valid_checkbox_count >= threshold
}

/// Detect if a column is a single-select field.
/// Optimizes by checking for repeated values across cells and using early exit strategies.
fn is_single_select_field(cells: &[&str]) -> bool {
  let mut value_counts: HashMap<&str, usize> = HashMap::new();

  for &cell in cells {
    // Early exit if multi-value cell is detected
    if cell.contains(',') {
      return false;
    }

    let trimmed_cell = cell.trim();
    *value_counts.entry(trimmed_cell).or_insert(0) += 1;
  }

  // Check if values are reused across multiple cells
  value_counts.values().any(|&count| count > 1)
}

/// Detect if a column is a multi-select field.
/// Optimizes by checking if values in different cells are reused.
fn is_multi_select_field(cells: &[&str]) -> bool {
  let mut value_counts: HashMap<&str, usize> = HashMap::new();

  for &cell in cells {
    // Split the cell by commas (indicating multiple values)
    let values: Vec<&str> = cell.split(',').map(|s| s.trim()).collect();

    // Count occurrences of each value across cells
    for value in values {
      *value_counts.entry(value).or_insert(0) += 1;
    }
  }

  // If any value appears more than once across different cells, it's likely multi-select
  value_counts.values().any(|&count| count > 1)
}

fn is_link_field(cells: &[&str]) -> bool {
  cells
    .iter()
    .all(|cell| cell.starts_with("http://") || cell.starts_with("https://"))
}

fn is_number_cell(cells: &[&str]) -> bool {
  let all_count = cells.len();
  let valid_count = cells
    .iter()
    .filter(|&&cell| cell.parse::<f64>().is_ok())
    .count();

  if valid_count == 0 {
    return false;
  }

  valid_count >= all_count
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_detect_field_type_url() {
    let cells = vec![
      "http://example.com",
      "https://example.org",
      "http://test.com",
    ];
    assert_eq!(detect_field_type_from_cells(&cells), FieldType::URL);
  }

  #[test]
  fn test_detect_field_type_single_select() {
    let cells = vec!["Done", "Not Started", "In Progress", "Done", "Not Started"];
    assert_eq!(
      detect_field_type_from_cells(&cells),
      FieldType::SingleSelect
    );
  }

  #[test]
  fn test_detect_field_type_multi_select() {
    let cells = vec![
      "Done, Not Started",
      "In Progress, Done",
      "Not Started, Done",
    ];
    assert_eq!(detect_field_type_from_cells(&cells), FieldType::MultiSelect);
  }

  #[test]
  fn test_detect_field_type_checkbox() {
    let cells = vec!["yes", "no", "no", "yes", "no", "no", "yes"];
    assert_eq!(detect_field_type_from_cells(&cells), FieldType::Checkbox);
  }

  #[test]
  fn test_is_checkbox_cell_invalid_value() {
    let cells = vec!["Yes", "No", "Maybe"];
    assert!(!is_checkbox_cell(&cells));
  }

  #[test]
  fn test_detect_field_type_datetime() {
    let cells = vec![
      "2023-05-21",
      "2023-06-11",
      "2023/07/12",
      "August 13, 2023",
      "12/09/2023",
    ];
    assert_eq!(detect_field_type_from_cells(&cells), FieldType::DateTime);
  }

  #[test]
  fn test_detect_field_type_rich_text() {
    let cells = vec!["This is a text", "Another text", "Some random content"];
    assert_eq!(detect_field_type_from_cells(&cells), FieldType::RichText);
  }

  #[test]
  fn test_is_link_field() {
    let cells = vec!["http://example.com", "https://example.org"];
    assert!(is_link_field(&cells));

    let cells = vec!["example.com", "https://example.org"];
    assert!(!is_link_field(&cells));
  }

  #[test]
  fn test_is_single_select_field() {
    let cells = vec!["Done", "Not Started", "In Progress", "Done"];
    assert!(is_single_select_field(&cells));

    let cells = vec!["Done, Not Started", "In Progress"];
    assert!(!is_single_select_field(&cells));
  }

  #[test]
  fn test_is_multi_select_field() {
    let cells = vec!["Done, Not Started", "In Progress, Done"];
    assert!(is_multi_select_field(&cells));

    let cells = vec!["Done", "Not Started", "In Progress"];
    assert!(!is_multi_select_field(&cells));
  }

  #[test]
  fn test_is_checkbox_cell() {
    let cells = vec!["Yes", "No", "1", "0", "true", "false", "yes", "no"];
    assert!(is_checkbox_cell(&cells));

    let cells = vec!["Yes", "No", "Maybe"];
    assert!(!is_checkbox_cell(&cells));
  }

  #[test]
  fn test_is_date_cell() {
    let cells = vec![
      "2023-05-21",
      "2023-06-11",
      "2023/07/12",
      "August 13, 2023",
      "12/09/2023",
    ];
    assert!(is_date_cell(&cells));

    let cells = vec!["2023-05-21", "Invalid Date", "12/09/2023"];
    assert!(is_date_cell(&cells));
  }
}
