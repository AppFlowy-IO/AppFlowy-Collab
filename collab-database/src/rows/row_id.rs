use std::fmt::{Display, Formatter};
use std::ops::Deref;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RowId(String);

impl Display for RowId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0.to_string())
  }
}

impl RowId {
  pub fn into_inner(self) -> String {
    self.0
  }
}

// impl Serialize for RowId {
//   fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//   where
//     S: Serializer,
//   {
//     serializer.serialize_str(&self.0.to_string())
//   }
// }

// impl<'de> Deserialize<'de> for RowId {
//   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//   where
//     D: Deserializer<'de>,
//   {
//     struct RowIdVisitor();
//
//     impl<'de> Visitor<'de> for RowIdVisitor {
//       type Value = RowId;
//
//       fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//         formatter.write_str("Expected i64 string")
//       }
//
//       fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
//       where
//         E: Error,
//       {
//         match v.parse::<i64>() {
//           Ok(id) => Ok(RowId(id)),
//           Err(_) => Err(Error::custom("Expected i64 string")),
//         }
//       }
//     }
//
//     deserializer.deserialize_any(RowIdVisitor())
//   }
// }

impl Deref for RowId {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for RowId {
  fn from(data: String) -> Self {
    Self(data)
  }
}

impl From<RowId> for String {
  fn from(data: RowId) -> Self {
    data.0
  }
}

impl From<i32> for RowId {
  fn from(data: i32) -> Self {
    Self(data.to_string())
  }
}

impl From<i64> for RowId {
  fn from(data: i64) -> Self {
    Self(data.to_string())
  }
}

impl From<usize> for RowId {
  fn from(data: usize) -> Self {
    Self(data.to_string())
  }
}

impl AsRef<str> for RowId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}
