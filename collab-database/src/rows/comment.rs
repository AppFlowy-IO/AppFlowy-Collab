use collab::preclude::{lib0Any};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RowComment {
  uid: i64,
  content: String,
  created_at: i64,
}

impl TryFrom<lib0Any> for RowComment {
  type Error = anyhow::Error;

  fn try_from(value: lib0Any) -> Result<Self, Self::Error> {
    let mut json = String::new();
    value.to_json(&mut json);
    let comment = serde_json::from_str(&json)?;
    Ok(comment)
  }
}

impl From<RowComment> for lib0Any {
  fn from(item: RowComment) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}
