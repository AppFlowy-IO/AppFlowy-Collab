use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

use collab::preclude::{MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::database::timestamp;
use crate::id_gen::ROW_ID_GEN;
use crate::rows::{Cell, Cells, CellsUpdate};
use crate::views::RowOrder;
use crate::{impl_bool_update, impl_i32_update, impl_i64_update};

#[derive(Copy, Debug, Clone, Eq, PartialEq, Hash)]
pub struct RowId(i64);

impl Display for RowId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0.to_string())
  }
}

impl Serialize for RowId {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.0.to_string())
  }
}

impl<'de> Deserialize<'de> for RowId {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct RowIdVisitor();

    impl<'de> Visitor<'de> for RowIdVisitor {
      type Value = RowId;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expected i64 string")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: Error,
      {
        match v.parse::<i64>() {
          Ok(id) => Ok(RowId(id)),
          Err(_) => Err(Error::custom("Expected i64 string")),
        }
      }
    }

    deserializer.deserialize_any(RowIdVisitor())
  }
}

impl Deref for RowId {
  type Target = i64;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<i64> for RowId {
  fn from(data: i64) -> Self {
    Self(data)
  }
}

impl From<RowId> for i64 {
  fn from(data: RowId) -> Self {
    data.0
  }
}

impl std::default::Default for RowId {
  fn default() -> Self {
    Self(ROW_ID_GEN.lock().next_id())
  }
}

impl AsRef<i64> for RowId {
  fn as_ref(&self) -> &i64 {
    &self.0
  }
}
