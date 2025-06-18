use anyhow::{Error, anyhow};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::define::{
  DATABASE, DATABASE_ID, DATABASE_INLINE_VIEW, DATABASE_METAS, DATABASE_ROW_DATA, DATABASE_ROW_ID,
  DOCUMENT_ROOT, FOLDER, FOLDER_META, FOLDER_WORKSPACE_ID, USER_AWARENESS, WORKSPACE_DATABASES,
};
use crate::proto;
use collab::preclude::{ArrayRef, Collab, MapExt, MapRef};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// The type of the collab object. It will be used to determine what kind of services should be
/// used to handle the object.
/// The value of the enum can't be changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize_repr, Deserialize_repr, Hash)]
#[repr(i32)]
pub enum CollabType {
  Document = 0,
  Database = 1,
  WorkspaceDatabase = 2,
  Folder = 3,
  DatabaseRow = 4,
  UserAwareness = 5,
  /// This type is used when the specific nature of the collaboration object is not recognized.
  /// It might represent an uninitialized state or a custom object not covered by existing types.
  ///
  /// No strict validation is applied when handling objects of this type(check out the [CollabType::validate_require_data]
  /// for more information), which means errors might not be caught as strictly as with known types.
  Unknown = 6,
}

#[derive(Debug, thiserror::Error)]
pub enum CollabValidateError {
  #[error("No required data: {0}")]
  NoRequiredData(String),
}

impl CollabType {
  pub fn value(&self) -> i32 {
    *self as i32
  }

  pub fn awareness_enabled(&self) -> bool {
    matches!(self, CollabType::Document)
  }

  pub fn indexed_enabled(&self) -> bool {
    matches!(self, CollabType::Document)
  }

  pub fn is_unknown(&self) -> bool {
    matches!(self, CollabType::Unknown)
  }

  /// Validates the provided collaboration object (`collab`) based on its type.
  ///
  /// checks for the presence of required data in the collaboration object
  /// to ensure it adheres to the expected structure for its type. The validation criteria
  /// vary depending on the `CollabType`.
  ///
  /// # Arguments
  /// - `collab`: A reference to the `Collab` object to validate.
  ///
  /// # Returns
  /// - `Ok(())` if the collab object contains all the required data for its type.
  /// - `Err(Error)` if the required data is missing or if the collab object does not meet
  ///   the validation criteria for its type.
  pub fn validate_require_data(&self, collab: &Collab) -> Result<(), CollabValidateError> {
    let txn = collab.transact();
    match self {
      CollabType::Document => {
        let _: MapRef = collab
          .data
          .get_with_path(&txn, [DOCUMENT_ROOT])
          .ok_or_else(|| no_required_data_error(self, DOCUMENT_ROOT))?;
        Ok(())
      },
      CollabType::Database => {
        let database: MapRef = collab
          .data
          .get_with_path(&txn, [DATABASE])
          .ok_or_else(|| no_required_data_error(self, DATABASE))?;

        let _: String = database
          .get_with_txn(&txn, DATABASE_ID)
          .ok_or_else(|| no_required_data_error(self, DATABASE_ID))?;

        let database_meta: MapRef = database
          .get_with_txn(&txn, DATABASE_METAS)
          .ok_or_else(|| no_required_data_error(self, DATABASE_METAS))?;

        let _: String = database_meta
          .get_with_txn(&txn, DATABASE_INLINE_VIEW)
          .ok_or_else(|| no_required_data_error(self, "database inline view id"))?;

        Ok(())
      },
      CollabType::WorkspaceDatabase => {
        let _: ArrayRef = collab
          .data
          .get_with_path(&txn, [WORKSPACE_DATABASES])
          .ok_or_else(|| no_required_data_error(self, WORKSPACE_DATABASES))?;
        Ok(())
      },
      CollabType::Folder => {
        let meta: MapRef = collab
          .data
          .get_with_path(&txn, [FOLDER, FOLDER_META])
          .ok_or_else(|| no_required_data_error(self, FOLDER_META))?;
        let current_workspace: String = meta
          .get_with_txn(&txn, FOLDER_WORKSPACE_ID)
          .ok_or_else(|| no_required_data_error(self, FOLDER_WORKSPACE_ID))?;

        if current_workspace.is_empty() {
          Err(no_required_data_error(self, FOLDER_WORKSPACE_ID))
        } else {
          Ok(())
        }
      },
      CollabType::DatabaseRow => {
        let row_map: MapRef = collab
          .data
          .get_with_path(&txn, [DATABASE_ROW_DATA])
          .ok_or_else(|| no_required_data_error(self, DATABASE_ROW_DATA))?;

        let _: String = row_map
          .get_with_txn(&txn, DATABASE_ROW_ID)
          .ok_or_else(|| no_required_data_error(self, DATABASE_ROW_ID))?;
        Ok(())
      },
      CollabType::UserAwareness => {
        let _: MapRef = collab
          .data
          .get_with_path(&txn, [USER_AWARENESS])
          .ok_or_else(|| no_required_data_error(self, USER_AWARENESS))?;
        Ok(())
      },
      CollabType::Unknown => Ok(()),
    }
  }
  pub fn from_proto(proto: &proto::CollabType) -> Self {
    match proto {
      proto::CollabType::Unknown => CollabType::Unknown,
      proto::CollabType::Document => CollabType::Document,
      proto::CollabType::Database => CollabType::Database,
      proto::CollabType::WorkspaceDatabase => CollabType::WorkspaceDatabase,
      proto::CollabType::Folder => CollabType::Folder,
      proto::CollabType::DatabaseRow => CollabType::DatabaseRow,
      proto::CollabType::UserAwareness => CollabType::UserAwareness,
    }
  }

  pub fn to_proto(&self) -> proto::CollabType {
    match self {
      CollabType::Unknown => proto::CollabType::Unknown,
      CollabType::Document => proto::CollabType::Document,
      CollabType::Database => proto::CollabType::Database,
      CollabType::WorkspaceDatabase => proto::CollabType::WorkspaceDatabase,
      CollabType::Folder => proto::CollabType::Folder,
      CollabType::DatabaseRow => proto::CollabType::DatabaseRow,
      CollabType::UserAwareness => proto::CollabType::UserAwareness,
    }
  }
}

/// Validates the workspace ID for 'Folder' type collaborations.
/// Ensures that the workspace ID contained in each Folder matches the expected workspace ID.
/// A mismatch indicates that the Folder data may be incorrect, potentially due to it being
/// overridden with data from another Folder.
pub fn validate_data_for_folder(collab: &Collab, workspace_id: &str) -> Result<(), Error> {
  let txn = collab.transact();
  let workspace_id_in_collab: String = collab
    .data
    .get_with_path(&txn, [FOLDER, FOLDER_META, FOLDER_WORKSPACE_ID])
    .ok_or_else(|| anyhow!("No required data: FOLDER_WORKSPACE_ID"))?;
  drop(txn);

  if workspace_id != workspace_id_in_collab {
    return Err(anyhow!(
      "Workspace ID mismatch: expected {}, but received {}",
      workspace_id,
      workspace_id_in_collab
    ));
  }
  Ok(())
}

#[inline]
fn no_required_data_error(collab_type: &CollabType, reason: &str) -> CollabValidateError {
  CollabValidateError::NoRequiredData(format!("{}:{}", collab_type, reason))
}

impl Display for CollabType {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Document => f.write_str("Document"),
      Self::Database => f.write_str("Database"),
      Self::WorkspaceDatabase => f.write_str("WorkspaceDatabase"),
      Self::DatabaseRow => f.write_str("DatabaseRow"),
      Self::Folder => f.write_str("Folder"),
      Self::UserAwareness => f.write_str("UserAwareness"),
      Self::Unknown => f.write_str("Unknown"),
    }
  }
}

macro_rules! impl_from_integer_for_collab_type {
  ($($t:ty),+) => {
    $(
      impl From<$t> for CollabType {
        fn from(n: $t) -> CollabType {
          match n {
              0 => CollabType::Document,
              1 => CollabType::Database,
              2 => CollabType::WorkspaceDatabase,
              3 => CollabType::Folder,
              4 => CollabType::DatabaseRow,
              5 => CollabType::UserAwareness,
              _ => CollabType::Unknown,
          }
        }
      }
    )+
  };
}

macro_rules! impl_from_collab_type_for_integer {
    ($($t:ty),+) => {
      $(
        impl From<CollabType> for $t {
          fn from(ct: CollabType) -> $t {
            match ct {
                CollabType::Document => 0,
                CollabType::Database => 1,
                CollabType::WorkspaceDatabase => 2,
                CollabType::Folder => 3,
                CollabType::DatabaseRow => 4,
                CollabType::UserAwareness => 5,
                CollabType::Unknown => 255,
            }
          }
        }
      )+
    };
}

impl_from_integer_for_collab_type!(i32, u8);
impl_from_collab_type_for_integer!(i32, u8);

#[derive(Clone, Debug)]
pub struct CollabObject {
  pub object_id: String,
  pub uid: i64,
  pub collab_type: CollabType,
  pub device_id: String,
  pub workspace_id: String,
  pub meta: HashMap<String, String>,
}

impl CollabObject {
  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    workspace_id: String,
    device_id: String,
  ) -> Self {
    Self {
      object_id,
      uid,
      collab_type,
      workspace_id,
      device_id,
      meta: Default::default(),
    }
  }

  pub fn with_meta(mut self, key: &str, value: String) -> Self {
    self.meta.insert(key.to_string(), value);
    self
  }
}

impl Display for CollabObject {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{:?}:{}]", self.collab_type, self.object_id,))
  }
}
