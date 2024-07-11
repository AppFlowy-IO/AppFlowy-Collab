use anyhow::{anyhow, Error};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::define::{
  DATABASE, DATABASE_ID, DATABASE_ROW_DATA, DOCUMENT_ROOT, FOLDER, FOLDER_META,
  FOLDER_WORKSPACE_ID, USER_AWARENESS, WORKSPACE_DATABASES,
};
use crate::proto;
use collab::preclude::{Collab, MapRefExtension};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// The type of the collab object. It will be used to determine what kind of services should be
/// used to handle the object.
/// The value of the enum can't be changed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize_repr, Deserialize_repr, Hash)]
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

impl CollabType {
  pub fn value(&self) -> i32 {
    self.clone() as i32
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
  pub fn validate_require_data(&self, collab: &Collab) -> Result<(), Error> {
    let txn = collab.try_transaction()?;
    match self {
      CollabType::Document => {
        collab
          .get_map_with_txn(&txn, vec![DOCUMENT_ROOT])
          .ok_or_else(|| no_required_data_error(self, DOCUMENT_ROOT))?;
        Ok(())
      },
      CollabType::Database => {
        let database = collab
          .get_map_with_txn(&txn, vec![DATABASE])
          .ok_or_else(|| no_required_data_error(self, DATABASE))?;

        database
          .get_str_with_txn(&txn, DATABASE_ID)
          .ok_or_else(|| no_required_data_error(self, DATABASE_ID))?;
        Ok(())
      },
      CollabType::WorkspaceDatabase => {
        let _ = collab
          .get_array_with_txn(&txn, vec![WORKSPACE_DATABASES])
          .ok_or_else(|| no_required_data_error(self, WORKSPACE_DATABASES))?;
        Ok(())
      },
      CollabType::Folder => {
        let meta = collab
          .get_map_with_txn(&txn, vec![FOLDER, FOLDER_META])
          .ok_or_else(|| no_required_data_error(self, FOLDER_META))?;
        let current_workspace = meta
          .get_str_with_txn(&txn, FOLDER_WORKSPACE_ID)
          .ok_or_else(|| no_required_data_error(self, FOLDER_WORKSPACE_ID))?;

        if current_workspace.is_empty() {
          Err(no_required_data_error(self, FOLDER_WORKSPACE_ID))
        } else {
          Ok(())
        }
      },
      CollabType::DatabaseRow => {
        collab
          .get_map_with_txn(&txn, vec![DATABASE_ROW_DATA])
          .ok_or_else(|| no_required_data_error(self, DATABASE_ROW_DATA))?;
        Ok(())
      },
      CollabType::UserAwareness => {
        collab
          .get_map_with_txn(&txn, vec![USER_AWARENESS])
          .ok_or_else(|| no_required_data_error(self, USER_AWARENESS))?;
        Ok(())
      },
      CollabType::Unknown => Ok(()),
    }
  }

  pub fn from_proto(proto: proto::collab::CollabType) -> Self {
    match proto {
      proto::collab::CollabType::Unknown => CollabType::Unknown,
      proto::collab::CollabType::Document => CollabType::Document,
      proto::collab::CollabType::Database => CollabType::Database,
      proto::collab::CollabType::WorkspaceDatabase => CollabType::WorkspaceDatabase,
      proto::collab::CollabType::Folder => CollabType::Folder,
      proto::collab::CollabType::DatabaseRow => CollabType::DatabaseRow,
      proto::collab::CollabType::UserAwareness => CollabType::UserAwareness,
    }
  }
}

/// Validates the workspace ID for 'Folder' type collaborations.
/// Ensures that the workspace ID contained in each Folder matches the expected workspace ID.
/// A mismatch indicates that the Folder data may be incorrect, potentially due to it being
/// overridden with data from another Folder.
pub fn validate_data_for_folder(collab: &Collab, workspace_id: &str) -> Result<(), Error> {
  let txn = collab.transact();
  let workspace_id_in_collab = collab
    .get_map_with_txn(&txn, vec![FOLDER, FOLDER_META])
    .and_then(|map| map.get_str_with_txn(&txn, FOLDER_WORKSPACE_ID))
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
fn no_required_data_error(collab_type: &CollabType, reason: &str) -> Error {
  anyhow!("No required data: {}:{}", collab_type, reason)
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
