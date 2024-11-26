use serde::{Deserialize, Serialize};

use super::{TypeOptionData, TypeOptionDataBuilder};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChecklistTypeOption;

impl From<TypeOptionData> for ChecklistTypeOption {
  fn from(_data: TypeOptionData) -> Self {
    Self
  }
}

impl From<ChecklistTypeOption> for TypeOptionData {
  fn from(_data: ChecklistTypeOption) -> Self {
    TypeOptionDataBuilder::default()
  }
}
