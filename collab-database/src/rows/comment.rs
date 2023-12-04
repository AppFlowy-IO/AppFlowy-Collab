use collab::preclude::Any;
use collab::util::deserialize_i64_from_numeric;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RowComment {
  uid: i64,
  content: String,
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  created_at: i64,
}

impl TryFrom<Any> for RowComment {
  type Error = anyhow::Error;

  fn try_from(value: Any) -> Result<Self, Self::Error> {
    let mut json = String::new();
    value.to_json(&mut json);
    let comment = serde_json::from_str(&json)?;
    Ok(comment)
  }
}

impl From<RowComment> for Any {
  fn from(item: RowComment) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    Any::from_json(&json).unwrap()
  }
}
