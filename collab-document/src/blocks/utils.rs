use crate::error::DocumentError;
use serde_json::Value;
use std::collections::HashMap;

pub fn json_str_to_hashmap(json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
  let v = serde_json::from_str(json_str);
  v.map_err(|_| DocumentError::ConvertDataError)
}

pub fn hashmap_to_json_str(data: HashMap<String, Value>) -> Result<String, DocumentError> {
  let v = serde_json::to_string(&data);
  v.map_err(|_| DocumentError::ConvertDataError)
}
