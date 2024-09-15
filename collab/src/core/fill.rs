use crate::util::ArrayExt;

use yrs::types::TypeRef;
use yrs::{Any, Array, ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, SharedRef, TransactionMut};

#[derive(Debug, thiserror::Error)]
pub enum FillError {
  #[error("cannot fill {0} with: {0}")]
  InvalidData(TypeRef, String),
}

/// Trait that allows to fill shared refs with data.
pub trait FillRef<R>
where
  R: SharedRef,
{
  fn fill(self, txn: &mut TransactionMut, shared_ref: &R) -> Result<(), FillError>;
}

impl FillRef<MapRef> for Any {
  fn fill(self, txn: &mut TransactionMut, shared_ref: &MapRef) -> Result<(), FillError> {
    match self {
      Any::Map(map) => {
        for (key, value) in map.iter() {
          let value = value.clone();
          match value {
            Any::Array(values) => {
              let nested_ref: ArrayRef =
                shared_ref.insert(txn, key.as_str(), ArrayPrelim::default());
              nested_ref.insert_range(txn, 0, values.to_vec());
            },
            value @ Any::Map(_) => {
              let nested_ref: MapRef = shared_ref.get_or_init(txn, key.as_str());
              value.fill(txn, &nested_ref)?;
            },
            other => {
              shared_ref.try_update(txn, key.as_str(), other);
            },
          }
        }
        Ok(())
      },
      _ => Err(FillError::InvalidData(TypeRef::Map, self.to_string())),
    }
  }
}

impl FillRef<ArrayRef> for Any {
  fn fill(self, txn: &mut TransactionMut, shared_ref: &ArrayRef) -> Result<(), FillError> {
    match self {
      Any::Array(array) => {
        shared_ref.clear(txn);
        for value in array.iter().cloned() {
          let map_ref = shared_ref.push_back(txn, MapPrelim::default());
          value.fill(txn, &map_ref)?;
        }
        Ok(())
      },
      _ => Err(FillError::InvalidData(TypeRef::Array, self.to_string())),
    }
  }
}
