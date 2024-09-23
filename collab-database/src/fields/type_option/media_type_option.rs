use crate::entity::FieldType;
use crate::fields::{TypeOptionData, TypeOptionDataBuilder};
use crate::rows::{new_cell_builder, Cell};
use crate::template::entity::CELL_DATA;
use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use yrs::Any;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaTypeOption {
  #[serde(default)]
  pub files: Vec<MediaFile>,

  #[serde(default)]
  pub hide_file_names: bool,
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct MediaCellData {
  pub files: Vec<MediaFile>,
}

impl From<&Cell> for MediaCellData {
  fn from(cell: &Cell) -> Self {
    let files = match cell.get(CELL_DATA) {
      Some(Any::Array(array)) => array
        .iter()
        .flat_map(|item| {
          if let Any::String(string) = item {
            Some(serde_json::from_str::<MediaFile>(string).unwrap_or_default())
          } else {
            None
          }
        })
        .collect(),
      _ => vec![],
    };

    Self { files }
  }
}

impl From<&MediaCellData> for Cell {
  fn from(value: &MediaCellData) -> Self {
    let data = Any::Array(Arc::from(
      value
        .files
        .clone()
        .into_iter()
        .map(|file| Any::String(Arc::from(serde_json::to_string(&file).unwrap_or_default())))
        .collect::<Vec<_>>(),
    ));

    let mut cell = new_cell_builder(FieldType::Media);
    cell.insert(CELL_DATA.into(), data);
    cell
  }
}

impl From<String> for MediaCellData {
  fn from(s: String) -> Self {
    if s.is_empty() {
      return MediaCellData { files: vec![] };
    }

    let files = s
      .split(", ")
      .map(|file: &str| serde_json::from_str::<MediaFile>(file).unwrap_or_default())
      .collect::<Vec<_>>();

    MediaCellData { files }
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

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Default, Clone)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MediaUploadType {
  #[default]
  LocalMedia = 0,
  NetworkMedia = 1,
  CloudMedia = 2,
}
