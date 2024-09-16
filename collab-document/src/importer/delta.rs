use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Delta {
  pub ops: Vec<Operation>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Operation {
  insert: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  attributes: Vec<(String, Value)>,
}

#[derive(Error, Debug)]
pub enum ConversionError {
  #[error("Invalid structure: expected an object")]
  NotAnObject,
  #[error("Missing 'insert' field")]
  MissingInsert,
  #[error("'insert' field is not a string")]
  InsertNotString,
  #[error("'attributes' field is not an object")]
  AttributesNotObject,
  #[error("Invalid attribute")]
  InvalidAttribute,
  #[error("Invalid insert")]
  InvalidInsert,
}

impl TryFrom<Value> for Operation {
  type Error = ConversionError;

  fn try_from(value: Value) -> Result<Self, Self::Error> {
    let obj = value.as_object().ok_or(ConversionError::NotAnObject)?;

    let insert = obj
      .get("insert")
      .ok_or(ConversionError::MissingInsert)?
      .as_str()
      .ok_or(ConversionError::InsertNotString)?
      .to_string();

    let attributes = obj
      .get("attributes")
      .map(|v| -> Result<HashMap<String, Value>, ConversionError> {
        v.as_object()
          .ok_or(ConversionError::AttributesNotObject)?
          .iter()
          .map(|(k, v)| Ok((k.clone(), v.clone())))
          .collect()
      })
      .transpose()?
      .unwrap_or_default();

    let attributes = attributes.into_iter().collect();
    Ok(Self { insert, attributes })
  }
}

impl TryFrom<Operation> for Value {
  type Error = ConversionError;

  fn try_from(op: Operation) -> Result<Self, Self::Error> {
    let attributes: HashMap<String, Value> = op.attributes.into_iter().collect();

    Ok(if attributes.is_empty() {
      json!({ "insert": op.insert })
    } else {
      json!({ "insert": op.insert, "attributes": attributes })
    })
  }
}

impl Delta {
  pub fn new() -> Self {
    Self { ops: Vec::new() }
  }

  pub fn insert(&mut self, value: String, attributes: Vec<(String, Value)>) {
    self.ops.push(Operation {
      insert: value,
      attributes,
    });
  }

  pub fn extend(&mut self, other: Delta) {
    self.ops.extend(other.ops);
  }

  pub fn to_json(&self) -> String {
    let ops: Vec<Value> = self
      .ops
      .iter()
      .filter_map(|op| Value::try_from(op.clone()).ok())
      .collect();

    serde_json::to_string(&ops).unwrap_or_else(|_| "[]".to_string())
  }
}
