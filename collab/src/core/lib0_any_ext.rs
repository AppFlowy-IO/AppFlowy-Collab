use crate::preclude::lib0Any;
use std::collections::HashMap;

pub trait Lib0AnyMapExtension {
  fn value(&self) -> &HashMap<String, lib0Any>;

  fn get_str_value<K: AsRef<str>>(&self, key: &K) -> Option<String> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::String(s) = value {
      Some(s.to_string())
    } else {
      None
    }
  }

  fn get_i64_value<K: AsRef<str>>(&self, key: &K) -> Option<i64> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::BigInt(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  fn get_bool_value<K: AsRef<str>>(&self, key: &K) -> Option<bool> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::Bool(value) = value {
      Some(*value)
    } else {
      None
    }
  }
}
