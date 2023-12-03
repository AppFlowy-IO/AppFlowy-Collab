use crate::preclude::YrsValue;
use lib0::any::Any;

pub trait YrsValueExtension {
  fn value(&self) -> &YrsValue;
  fn as_i64(&self) -> Option<i64> {
    if let YrsValue::Any(any) = self.value() {
      if let Any::BigInt(value) = any {
        return Some(*value);
      }
    }
    None
  }

  fn as_str(&self) -> Option<&str> {
    if let YrsValue::Any(any) = self.value() {
      if let Any::String(value) = any {
        return Some(value);
      }
    }
    None
  }

  fn as_bool(&self) -> Option<bool> {
    if let YrsValue::Any(any) = self.value() {
      if let Any::Bool(value) = any {
        return Some(*value);
      }
    }
    None
  }

  fn as_f64(&self) -> Option<f64> {
    if let YrsValue::Any(any) = self.value() {
      if let Any::Number(value) = any {
        return Some(*value);
      }
    }
    None
  }
}

impl YrsValueExtension for YrsValue {
  fn value(&self) -> &YrsValue {
    self
  }
}
