use collab::util::AnyMapExt;
use serde::{Deserialize, Serialize};

use super::{TypeOptionData, TypeOptionDataBuilder};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummarizationTypeOption {
  pub auto_fill: bool,
}

impl From<TypeOptionData> for SummarizationTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let auto_fill: bool = data.get_as("auto_fill").unwrap_or_default();
    Self { auto_fill }
  }
}

impl From<SummarizationTypeOption> for TypeOptionData {
  fn from(data: SummarizationTypeOption) -> Self {
    TypeOptionDataBuilder::from([("auto_fill".into(), data.auto_fill.into())])
  }
}
