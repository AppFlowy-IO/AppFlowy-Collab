use serde_json::Value;
use yrs::block::{ItemContent, Prelim, Unused};
use yrs::branch::{Branch, BranchPtr};
use yrs::types::TypeRef;
use yrs::{Any, Array, ArrayRef, Map, MapRef, TransactionMut};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Entity(Value);

impl From<Value> for Entity {
  fn from(value: Value) -> Self {
    Entity(value)
  }
}

impl Prelim for Entity {
  type Return = Unused;

  fn into_content(self, _txn: &mut TransactionMut) -> (ItemContent, Option<Self>) {
    match &self.0 {
      Value::Null => (ItemContent::Any(vec![Any::Null]), None),
      Value::Bool(value) => (ItemContent::Any(vec![Any::from(*value)]), None),
      Value::String(value) => (ItemContent::Any(vec![Any::from(value.clone())]), None),
      Value::Number(value) => {
        let any = if value.is_f64() {
          Any::from(value.as_f64().unwrap())
        } else {
          Any::from(value.as_i64().unwrap())
        };
        (ItemContent::Any(vec![any]), None)
      },
      Value::Array(_) => {
        let yarray = ItemContent::Type(Branch::new(TypeRef::Array));
        (yarray, Some(self))
      },
      Value::Object(_) => {
        let yarray = ItemContent::Type(Branch::new(TypeRef::Map));
        (yarray, Some(self))
      },
    }
  }

  fn integrate(self, txn: &mut TransactionMut, inner_ref: BranchPtr) {
    match self.0 {
      Value::Array(array) => {
        let yarray = ArrayRef::from(inner_ref);
        for value in array {
          yarray.push_back(txn, Entity::from(value));
        }
      },
      Value::Object(map) => {
        let ymap = MapRef::from(inner_ref);
        for (key, value) in map {
          ymap.insert(txn, key, Entity::from(value));
        }
      },
      _ => { /* not used */ },
    }
  }
}
