use std::collections::HashMap;
use std::sync::Arc;
use yrs::Any;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AnyMut {
  #[default]
  Null,
  Bool(bool),
  Number(f64),
  BigInt(i64),
  String(String),
  Bytes(bytes::BytesMut),
  Array(Vec<AnyMut>),
  Map(HashMap<String, AnyMut>),
}

impl From<Any> for AnyMut {
  fn from(value: Any) -> Self {
    match value {
      Any::Null => AnyMut::Null,
      Any::Undefined => AnyMut::Null,
      Any::Bool(bool) => AnyMut::Bool(bool),
      Any::Number(num) => AnyMut::Number(num),
      Any::BigInt(num) => AnyMut::BigInt(num),
      Any::String(str) => AnyMut::String(str.to_string()),
      Any::Buffer(buf) => AnyMut::Bytes(bytes::BytesMut::from(&*buf)),
      Any::Array(array) => {
        let array: Vec<AnyMut> = array.iter().map(|any| AnyMut::from(any.clone())).collect();
        AnyMut::Array(array)
      },
      Any::Map(map) => {
        let owned = Arc::try_unwrap(map).unwrap_or_else(|map| (*map).clone());
        let map: HashMap<String, AnyMut> = owned
          .into_iter()
          .map(|(k, v)| (k, AnyMut::from(v)))
          .collect();
        AnyMut::Map(map)
      },
    }
  }
}

impl From<AnyMut> for Any {
  fn from(value: AnyMut) -> Self {
    match value {
      AnyMut::Null => Any::Null,
      AnyMut::Bool(bool) => Any::Bool(bool),
      AnyMut::Number(num) => Any::Number(num),
      AnyMut::BigInt(num) => Any::BigInt(num),
      AnyMut::String(str) => Any::String(str.into()),
      AnyMut::Bytes(bytes) => Any::Buffer(bytes.freeze().to_vec().into()),
      AnyMut::Array(array) => Any::Array(array.into_iter().map(Any::from).collect()),
      AnyMut::Map(map) => {
        let map: HashMap<String, Any> = map.into_iter().map(|(k, v)| (k, Any::from(v))).collect();
        Any::Map(map.into())
      },
    }
  }
}
