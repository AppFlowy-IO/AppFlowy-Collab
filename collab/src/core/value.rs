use crate::preclude::YrsValue;
use yrs::{Any, ArrayRef, MapRef, TextRef};

pub trait YrsValueExtension {
  fn value(&self) -> &YrsValue;

  fn to_ymap(&self) -> Option<&MapRef> {
    if let YrsValue::YMap(map) = self.value() {
      return Some(map);
    }
    None
  }

  fn to_yarray(&self) -> Option<&ArrayRef> {
    if let YrsValue::YArray(array) = self.value() {
      return Some(array);
    }
    None
  }

  fn to_ytext(&self) -> Option<&TextRef> {
    if let YrsValue::YText(text) = self.value() {
      return Some(text);
    }
    None
  }

  fn as_i64(&self) -> Option<i64> {
    if let YrsValue::Any(Any::BigInt(value)) = self.value() {
      return Some(*value);
    }
    None
  }

  fn as_str(&self) -> Option<&str> {
    if let YrsValue::Any(Any::String(value)) = self.value() {
      return Some(value);
    }
    None
  }

  fn as_bool(&self) -> Option<bool> {
    if let YrsValue::Any(Any::Bool(value)) = self.value() {
      return Some(*value);
    }
    None
  }

  fn as_f64(&self) -> Option<f64> {
    if let YrsValue::Any(Any::Number(value)) = self.value() {
      return Some(*value);
    }
    None
  }
}

impl YrsValueExtension for YrsValue {
  fn value(&self) -> &YrsValue {
    self
  }
}
