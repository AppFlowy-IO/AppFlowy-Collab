use std::fmt::{Display, Formatter};
use std::ops::Deref;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FieldId(String);

impl Display for FieldId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0.to_string())
  }
}

impl FieldId {
  pub fn into_inner(self) -> String {
    self.0
  }
}

impl Deref for FieldId {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for FieldId {
  fn from(data: String) -> Self {
    Self(data)
  }
}

impl From<FieldId> for String {
  fn from(data: FieldId) -> Self {
    data.0
  }
}

impl From<i32> for FieldId {
  fn from(data: i32) -> Self {
    Self(data.to_string())
  }
}

impl From<i64> for FieldId {
  fn from(data: i64) -> Self {
    Self(data.to_string())
  }
}

impl From<usize> for FieldId {
  fn from(data: usize) -> Self {
    Self(data.to_string())
  }
}

impl AsRef<str> for FieldId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}
