use crate::database::gen_database_file_id;
use crate::entity::FieldType;
use crate::fields::{
  TypeOptionCellReader, TypeOptionCellWriter, TypeOptionData, TypeOptionDataBuilder,
};
use crate::rows::{Cell, new_cell_builder};

use crate::error::DatabaseError;
use crate::template::entity::CELL_DATA;
use crate::template::util::{ToCellString, TypeOptionCellData};
use collab::util::AnyMapExt;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use serde_repr::Serialize_repr;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use yrs::Any;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTypeOption {
  pub hide_file_names: bool,
}

impl Default for MediaTypeOption {
  fn default() -> Self {
    Self {
      hide_file_names: true,
    }
  }
}

impl TypeOptionCellReader for MediaTypeOption {
  fn json_cell(&self, cell: &Cell) -> Value {
    match cell.get_as::<MediaCellData>(CELL_DATA) {
      None => Value::Null,
      Some(s) => json!(s),
    }
  }

  fn numeric_cell(&self, cell: &Cell) -> Option<f64> {
    let cell_data = cell.get_as::<String>(CELL_DATA)?;
    cell_data.parse::<f64>().ok()
  }

  fn convert_raw_cell_data(&self, text: &str) -> String {
    match serde_json::from_str::<MediaCellData>(text) {
      Ok(value) => value.to_cell_string(),
      Err(_) => "".to_string(),
    }
  }
}

impl TypeOptionCellWriter for MediaTypeOption {
  fn convert_json_to_cell(&self, json_value: Value) -> Cell {
    let cell_data = serde_json::from_value::<MediaCellData>(json_value).unwrap_or_default();
    cell_data.into()
  }
}

impl From<TypeOptionData> for MediaTypeOption {
  fn from(data: TypeOptionData) -> Self {
    data
      .get_as::<String>("content")
      .map(|s| serde_json::from_str::<MediaTypeOption>(&s).unwrap_or_default())
      .unwrap_or_default()
  }
}

impl From<MediaTypeOption> for TypeOptionData {
  fn from(data: MediaTypeOption) -> Self {
    let content = serde_json::to_string(&data).unwrap_or_default();
    TypeOptionDataBuilder::from([("content".into(), content.into())])
  }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaCellData {
  pub files: Vec<MediaFile>,
}

impl TypeOptionCellData for MediaCellData {
  fn is_cell_empty(&self) -> bool {
    self.files.is_empty()
  }
}

impl From<MediaCellData> for Any {
  fn from(data: MediaCellData) -> Self {
    Any::Array(Arc::from(
      data
        .files
        .clone()
        .into_iter()
        .map(|file| Any::String(Arc::from(serde_json::to_string(&file).unwrap_or_default())))
        .collect::<Vec<_>>(),
    ))
  }
}

impl TryFrom<Any> for MediaCellData {
  type Error = Any;

  fn try_from(value: Any) -> Result<Self, Self::Error> {
    match value {
      Any::Array(array) => {
        let files = array
          .iter()
          .flat_map(|item| {
            if let Any::String(string) = item {
              Some(serde_json::from_str::<MediaFile>(string).unwrap_or_default())
            } else {
              None
            }
          })
          .collect();
        Ok(Self { files })
      },
      _ => Ok(Self::default()),
    }
  }
}
impl From<&Cell> for MediaCellData {
  fn from(cell: &Cell) -> Self {
    cell.get_as::<MediaCellData>(CELL_DATA).unwrap_or_default()
  }
}

impl From<MediaCellData> for Cell {
  fn from(value: MediaCellData) -> Self {
    let mut cell = new_cell_builder(FieldType::Media);
    cell.insert(CELL_DATA.into(), value.into());
    cell
  }
}

impl ToCellString for MediaCellData {
  fn to_cell_string(&self) -> String {
    self
      .files
      .iter()
      .map(|file| file.name.clone())
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl FromStr for MediaCellData {
  type Err = DatabaseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.is_empty() {
      return Ok(MediaCellData { files: vec![] });
    }
    let files = s
      .split(", ")
      .map(|file: &str| serde_json::from_str::<MediaFile>(file).unwrap_or_default())
      .collect::<Vec<_>>();

    Ok(MediaCellData { files })
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaFile {
  pub id: String,
  pub name: String,
  pub url: String,
  pub upload_type: MediaUploadType,
  pub file_type: MediaFileType,
}

impl MediaFile {
  pub fn new(
    name: String,
    url: String,
    upload_type: MediaUploadType,
    file_type: MediaFileType,
  ) -> Self {
    Self {
      id: gen_database_file_id(),
      name,
      url,
      upload_type,
      file_type,
    }
  }

  pub fn rename(&self, new_name: String) -> Self {
    Self {
      id: self.id.clone(),
      name: new_name,
      url: self.url.clone(),
      upload_type: self.upload_type.clone(),
      file_type: self.file_type.clone(),
    }
  }
}

impl Display for MediaFile {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "MediaFile(id: {}, name: {}, url: {}, upload_type: {:?}, file_type: {:?})",
      self.id, self.name, self.url, self.upload_type, self.file_type
    )
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum MediaFileType {
  #[default]
  Other = 0,
  Image = 1,
  Link = 2,
  Document = 3,
  Archive = 4,
  Video = 5,
  Audio = 6,
  Text = 7,
}

impl MediaFileType {
  pub fn from_file<T: AsRef<Path>>(path: T) -> MediaFileType {
    match path
      .as_ref()
      .extension()
      .and_then(std::ffi::OsStr::to_str)
      .unwrap_or("")
      .to_lowercase()
      .as_str()
    {
      "jpg" | "jpeg" | "png" | "gif" => MediaFileType::Image,
      "zip" | "rar" | "tar" => MediaFileType::Archive,
      "mp4" | "mov" | "avi" => MediaFileType::Video,
      "mp3" | "wav" => MediaFileType::Audio,
      "txt" => MediaFileType::Text,
      "doc" | "docx" => MediaFileType::Document,
      "html" | "htm" => MediaFileType::Link,
      _ => MediaFileType::Other,
    }
  }
}
impl<'de> Deserialize<'de> for MediaFileType {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct MediaFileTypeVisitor;

    impl serde::de::Visitor<'_> for MediaFileTypeVisitor {
      type Value = MediaFileType;

      fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string or a u8 representing MediaFileType")
      }

      fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        match value {
          0 => Ok(MediaFileType::Other),
          1 => Ok(MediaFileType::Image),
          2 => Ok(MediaFileType::Link),
          3 => Ok(MediaFileType::Document),
          4 => Ok(MediaFileType::Archive),
          5 => Ok(MediaFileType::Video),
          6 => Ok(MediaFileType::Audio),
          7 => Ok(MediaFileType::Text),
          _ => Err(E::custom(format!(
            "Unknown numeric value for MediaFileType: {}",
            value
          ))),
        }
      }

      fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        match value {
          "Other" => Ok(MediaFileType::Other),
          "Image" => Ok(MediaFileType::Image),
          "Link" => Ok(MediaFileType::Link),
          "Document" => Ok(MediaFileType::Document),
          "Archive" => Ok(MediaFileType::Archive),
          "Video" => Ok(MediaFileType::Video),
          "Audio" => Ok(MediaFileType::Audio),
          "Text" => Ok(MediaFileType::Text),
          _ => Err(E::custom(format!(
            "Unknown string variant for MediaFileType: {}",
            value
          ))),
        }
      }
    }

    deserializer.deserialize_any(MediaFileTypeVisitor)
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum MediaUploadType {
  #[default]
  Local = 0,
  /// Network means file is external http URL
  Network = 1,
  /// Cloud means file stored in appflowy cloud
  Cloud = 2,
}

impl<'de> Deserialize<'de> for MediaUploadType {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct MediaUploadTypeVisitor;

    impl serde::de::Visitor<'_> for MediaUploadTypeVisitor {
      type Value = MediaUploadType;

      fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string or a u8 representing MediaUploadType")
      }

      fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        match value {
          0 => Ok(MediaUploadType::Local),
          1 => Ok(MediaUploadType::Network),
          2 => Ok(MediaUploadType::Cloud),
          _ => Err(E::custom(format!(
            "Unknown numeric value for MediaUploadType: {}",
            value
          ))),
        }
      }

      fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        match value {
          "Local" | "LocalMedia" => Ok(MediaUploadType::Local),
          "Network" | "NetworkMedia" => Ok(MediaUploadType::Network),
          "Cloud" | "CloudMedia" => Ok(MediaUploadType::Cloud),
          _ => Err(E::custom(format!(
            "Unknown string variant for MediaUploadType: {}",
            value
          ))),
        }
      }
    }

    deserializer.deserialize_any(MediaUploadTypeVisitor)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  #[test]
  fn test_is_cell_empty() {
    let empty_media_cell_data = MediaCellData { files: vec![] };
    assert!(empty_media_cell_data.is_cell_empty());

    let non_empty_media_cell_data = MediaCellData {
      files: vec![MediaFile::new(
        "file1.jpg".to_string(),
        "http://example.com/file1.jpg".to_string(),
        MediaUploadType::Local,
        MediaFileType::Image,
      )],
    };
    assert!(!non_empty_media_cell_data.is_cell_empty());
  }

  #[test]
  fn test_media_file_rename() {
    let original = MediaFile::new(
      "original_name.jpg".to_string(),
      "http://example.com/file.jpg".to_string(),
      MediaUploadType::Local,
      MediaFileType::Image,
    );

    let renamed = original.rename("new_name.jpg".to_string());
    assert_eq!(renamed.name, "new_name.jpg");
    assert_eq!(renamed.url, original.url);
    assert_eq!(renamed.upload_type, original.upload_type);
    assert_eq!(renamed.file_type, original.file_type);
  }

  #[test]
  fn test_invalid_json_deserialization() {
    let invalid_json = json!("InvalidType");
    assert!(serde_json::from_value::<MediaUploadType>(invalid_json.clone()).is_err());
    assert!(serde_json::from_value::<MediaFileType>(invalid_json).is_err());
  }

  #[test]
  fn test_media_cell_data_to_string() {
    let media_file_1 = MediaFile::new(
      "file1.jpg".to_string(),
      "http://example.com/file1.jpg".to_string(),
      MediaUploadType::Local,
      MediaFileType::Image,
    );
    let media_file_2 = MediaFile::new(
      "file2.png".to_string(),
      "http://example.com/file2.png".to_string(),
      MediaUploadType::Cloud,
      MediaFileType::Image,
    );

    let media_cell_data = MediaCellData {
      files: vec![media_file_1.clone(), media_file_2.clone()],
    };

    let expected = "file1.jpg, file2.png".to_string();
    assert_eq!(media_cell_data.to_cell_string(), expected);
  }

  #[test]
  fn test_media_file_type_from_file_extension() {
    assert_eq!(
      MediaFileType::from_file("example.jpg"),
      MediaFileType::Image
    );
    assert_eq!(
      MediaFileType::from_file("example.mp4"),
      MediaFileType::Video
    );
    assert_eq!(
      MediaFileType::from_file("example.unknown"),
      MediaFileType::Other
    );
  }

  #[test]
  fn test_serialize_deserialize_media_cell_data() {
    let media_file_1 = MediaFile::new(
      "file1.jpg".to_string(),
      "http://example.com/file1.jpg".to_string(),
      MediaUploadType::Local,
      MediaFileType::Image,
    );
    let media_file_2 = MediaFile::new(
      "file2.png".to_string(),
      "http://example.com/file2.png".to_string(),
      MediaUploadType::Cloud,
      MediaFileType::Image,
    );

    let media_cell_data = MediaCellData {
      files: vec![media_file_1.clone(), media_file_2.clone()],
    };

    // Serialize to JSON
    let serialized = serde_json::to_string(&media_cell_data).unwrap();
    println!("Serialized MediaCellData: {}", serialized);

    // Deserialize back to struct
    let deserialized: MediaCellData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(media_cell_data, deserialized);
  }

  #[test]
  fn test_media_file_display() {
    let media_file = MediaFile::new(
      "test_file.txt".to_string(),
      "http://example.com/file.txt".to_string(),
      MediaUploadType::Network,
      MediaFileType::Text,
    );

    let expected_display = format!(
      "MediaFile(id: {}, name: test_file.txt, url: http://example.com/file.txt, upload_type: {:?}, file_type: {:?})",
      media_file.id, media_file.upload_type, media_file.file_type
    );

    assert_eq!(media_file.to_string(), expected_display);
  }

  #[test]
  fn test_deserialize_media_upload_type() {
    let json_local = json!("Local");
    let json_network = json!(1);
    let json_cloud = json!("CloudMedia");

    assert_eq!(
      serde_json::from_value::<MediaUploadType>(json_local).unwrap(),
      MediaUploadType::Local
    );
    assert_eq!(
      serde_json::from_value::<MediaUploadType>(json_network).unwrap(),
      MediaUploadType::Network
    );
    assert_eq!(
      serde_json::from_value::<MediaUploadType>(json_cloud).unwrap(),
      MediaUploadType::Cloud
    );
  }

  #[test]
  fn test_deserialize_media_file_type() {
    let json_image = json!(1);
    let json_text = json!("Text");

    assert_eq!(
      serde_json::from_value::<MediaFileType>(json_image).unwrap(),
      MediaFileType::Image
    );
    assert_eq!(
      serde_json::from_value::<MediaFileType>(json_text).unwrap(),
      MediaFileType::Text
    );
  }

  #[test]
  fn test_convert_raw_cell_data() {
    let media_type_option = MediaTypeOption::default();

    // Test with valid JSON data
    let valid_data =
      r#"{"files":[{"id":"1","name":"file1","url":"url1","upload_type":0,"file_type":1}]}"#;
    assert_eq!(
      media_type_option.convert_raw_cell_data(valid_data),
      "file1".to_string()
    );

    // Test with invalid JSON data
    let invalid_data = "invalid_json";
    assert_eq!(
      media_type_option.convert_raw_cell_data(invalid_data),
      "".to_string()
    );

    // Test with empty string
    let empty_data = "";
    assert_eq!(
      media_type_option.convert_raw_cell_data(empty_data),
      "".to_string()
    );

    // Test with valid JSON but missing "files" field
    let missing_field_data = r#"{"other_field":[{"id":"1"}]}"#;
    assert_eq!(
      media_type_option.convert_raw_cell_data(missing_field_data),
      "".to_string()
    );

    // Test with valid JSON but incorrect structure
    let incorrect_structure_data = r#"{"files":"not_an_array"}"#;
    assert_eq!(
      media_type_option.convert_raw_cell_data(incorrect_structure_data),
      "".to_string()
    );
  }

  #[test]
  fn test_numeric_cell_conversion() {
    let mut cell = new_cell_builder(FieldType::Media);
    cell.insert(CELL_DATA.into(), "123.45".to_string().into());

    let media_type_option = MediaTypeOption::default();
    let numeric_value = media_type_option.numeric_cell(&cell);

    assert_eq!(numeric_value, Some(123.45));
  }

  #[test]
  fn test_media_cell_data_to_and_from_cell() {
    // Create MediaCellData with sample MediaFile entries
    let media_file_1 = MediaFile::new(
      "file1.jpg".to_string(),
      "http://example.com/file1.jpg".to_string(),
      MediaUploadType::Local,
      MediaFileType::Image,
    );
    let media_file_2 = MediaFile::new(
      "file2.png".to_string(),
      "http://example.com/file2.png".to_string(),
      MediaUploadType::Cloud,
      MediaFileType::Image,
    );

    let media_cell_data = MediaCellData {
      files: vec![media_file_1.clone(), media_file_2.clone()],
    };

    // Convert MediaCellData to a Cell
    let cell: Cell = media_cell_data.clone().into();

    // Assert the Cell has the correct field type and content
    let cell_data = MediaCellData::from(&cell);
    cell_data
      .files
      .iter()
      .zip(media_cell_data.files.iter())
      .for_each(|(a, b)| {
        assert_eq!(a, b);
      });
  }
}
