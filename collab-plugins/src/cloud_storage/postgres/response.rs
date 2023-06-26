use serde::{Deserialize, Deserializer};

#[allow(dead_code)]
#[derive(Deserialize)]
pub(crate) struct KeyValueListResponse(pub Vec<KeyValueResponse>);

#[allow(dead_code)]
#[derive(Deserialize)]
pub(crate) struct KeyValueResponse {
  #[serde(deserialize_with = "deserialize_null_or_default")]
  pub value: String,
}

/// Handles the case where the value is null. If the value is null, return the default value of the
/// type. Otherwise, deserialize the value.
fn deserialize_null_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
  T: Default + Deserialize<'de>,
  D: Deserializer<'de>,
{
  let opt = Option::deserialize(deserializer)?;
  Ok(opt.unwrap_or_default())
}
