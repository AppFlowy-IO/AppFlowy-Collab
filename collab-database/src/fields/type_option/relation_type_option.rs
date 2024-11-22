use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};

use super::{TypeOptionData, TypeOptionDataBuilder};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationTypeOption {
  pub database_id: String,
}

impl From<TypeOptionData> for RelationTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let database_id: String = data.get_as("database_id").unwrap_or_default();
    Self { database_id }
  }
}

impl From<RelationTypeOption> for TypeOptionData {
  fn from(data: RelationTypeOption) -> Self {
    TypeOptionDataBuilder::from([("database_id".into(), data.database_id.into())])
  }
}
