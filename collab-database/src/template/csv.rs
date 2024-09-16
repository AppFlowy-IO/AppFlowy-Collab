use crate::entity::FieldType;
use crate::error::DatabaseError;
use crate::template::builder::DatabaseTemplateBuilder;
use crate::template::entity::DatabaseTemplate;
use std::io;

pub struct CSVTemplate {
  pub fields: Vec<CSVField>,
  pub rows: Vec<Vec<String>>,
}

pub struct CSVField {
  name: String,
  field_type: FieldType,
}

impl CSVTemplate {
  pub fn try_from_reader(reader: impl io::Read) -> Result<Self, DatabaseError> {
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
      return Err(DatabaseError::InvalidCSV("No header".to_string()));
    }

    let rows: Vec<Vec<String>> = reader
      .records()
      .flat_map(|r| r.ok())
      .map(|record| {
        record
          .into_iter()
          .map(|s| s.to_string())
          .collect::<Vec<String>>()
      })
      .collect();

    Ok(CSVTemplate { fields, rows })
  }
}

impl TryFrom<CSVTemplate> for DatabaseTemplate {
  type Error = DatabaseError;

  fn try_from(value: CSVTemplate) -> Result<Self, Self::Error> {
    let mut builder = DatabaseTemplateBuilder::new();
    let CSVTemplate { fields, rows } = value;
    for (field_index, field) in fields.into_iter().enumerate() {
      builder = builder.create_field(
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
      );
    }

    Ok(builder.build())
  }
}

impl TryFrom<&[u8]> for CSVTemplate {
  type Error = DatabaseError;

  fn try_from(content: &[u8]) -> Result<Self, Self::Error> {
    Self::try_from_reader(content)
  }
}

impl TryFrom<String> for CSVTemplate {
  type Error = DatabaseError;

  fn try_from(content: String) -> Result<Self, Self::Error> {
    Self::try_from_reader(content.as_bytes())
  }
}

impl TryFrom<&str> for CSVTemplate {
  type Error = DatabaseError;

  fn try_from(content: &str) -> Result<Self, Self::Error> {
    Self::try_from_reader(content.as_bytes())
  }
}
