use crate::local_storage::kv::{KVTransactionDB, PersistenceError};
use std::path::Path;

pub struct KVTransactionDBIndexedDBImpl {}

impl KVTransactionDB for KVTransactionDBIndexedDBImpl {
  type TransactionAction<'a> = ();

  fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError>
  where
    Self: Sized,
  {
    todo!()
  }

  fn read_txn<'a>(&self) -> Self::TransactionAction<'a> {
    todo!()
  }

  fn with_write_txn<'a>(
    &self,
    f: impl FnOnce(&Self::TransactionAction<'a>) -> Result<(), PersistenceError>,
  ) -> Result<(), PersistenceError> {
    todo!()
  }

  fn flush(&self) -> Result<(), PersistenceError> {
    todo!()
  }
}
