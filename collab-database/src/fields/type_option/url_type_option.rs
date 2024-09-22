use crate::fields::{TypeOptionData, TypeOptionDataBuilder};
use collab::preclude::Any;
use serde::{Deserialize, Serialize};
use yrs::encoding::serde::from_any;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct URLTypeOption {
  #[serde(default)]
  pub url: String,
  #[serde(default)]
  pub content: String,
}

impl From<TypeOptionData> for URLTypeOption {
  fn from(data: TypeOptionData) -> Self {
    from_any(&Any::from(data)).unwrap()
  }
}

impl From<URLTypeOption> for TypeOptionData {
  fn from(data: URLTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("url".into(), data.url.into()),
      ("content".into(), data.content.into()),
    ])
  }
}
