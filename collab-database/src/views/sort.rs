use crate::fields::FieldType;
use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Sort {
  pub id: String,
  pub field_id: String,
  pub field_type: FieldType,
  pub condition: SortCondition,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, Clone, Debug)]
#[repr(u8)]
pub enum SortCondition {
  Ascending = 0,
  Descending = 1,
}

impl Default for SortCondition {
  fn default() -> Self {
    Self::Ascending
  }
}
