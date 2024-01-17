use crate::local_storage::kv::{KVEntry, KVStore, KVTransactionDB, PersistenceError};
use indexed_db_futures::js_sys::wasm_bindgen::JsValue;
use indexed_db_futures::prelude::*;
use rocksdb::{DBIteratorWithThreadMode, Transaction};
use std::ops::RangeBounds;

pub struct KVTransactionDBIndexedDBImpl {
  db: IdbDatabase,
}
const COLLAB_KV_STORE: &str = "collab_kv";
impl KVTransactionDBIndexedDBImpl {
  pub async fn new() -> Result<Self, PersistenceError> {
    let mut db_req = IdbDatabase::open_u32("appflowy_indexeddb", 1)?;
    db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
      if let None = evt.db().object_store_names().find(|n| n == COLLAB_KV_STORE) {
        evt.db().create_object_store(COLLAB_KV_STORE)?;
      }
      Ok(())
    }));
    let db = db_req.await?;
    // let tx = db.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    // let store = tx.object_store(COLLAB_KV_STORE)?;
    // let a = store.open_cursor().unwrap().await.unwrap().unwrap();
    // a.continue_cursor().unwrap();
    // let value_to_put = JsValue::null();
    // store.put_key_val_owned("my_key", &value_to_put)?;

    Ok(Self { db })
  }
}

impl KVTransactionDB for KVTransactionDBIndexedDBImpl {
  type TransactionAction<'a> = ();

  fn read_txn<'a, 'b>(&'b self) -> Self::TransactionAction<'a>
  where
    'b: 'a,
  {
    todo!()
  }

  fn with_write_txn<'a, 'b, Output>(
    &'b self,
    f: impl FnOnce(&Self::TransactionAction<'a>) -> Result<Output, PersistenceError>,
  ) -> Result<Output, PersistenceError>
  where
    'b: 'a,
  {
    todo!()
  }

  fn flush(&self) -> Result<(), PersistenceError> {
    todo!()
  }
}

pub struct IndexedDBKVStoreImpl<'a>(IdbTransaction<'a>);

impl<'a> IndexedDBKVStoreImpl<'a> {
  pub fn new(tx: IdbTransaction<'a>) -> Self {
    Self(tx)
  }
}

impl<'a> KVStore<'a> for IndexedDBKVStoreImpl<'a> {
  type Range = ();
  type Entry = ();
  type Value = ();
  type Error = ();

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    todo!()
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    todo!()
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    todo!()
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    todo!()
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    todo!()
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    todo!()
  }
}

pub struct IndexeddbRange<'a, DB> {
  // inner: IdbCursor<'a>,
  to: Vec<u8>,
}

impl<'a, DB: Send + Sync> Iterator for IndexeddbRange<'a, DB> {
  type Item = IndexeddbEntry;

  fn next(&mut self) -> Option<Self::Item> {
    None
  }
}

pub struct IndexeddbEntry {
  key: Vec<u8>,
  value: Vec<u8>,
}

impl IndexeddbEntry {
  pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
    Self { key, value }
  }
}

impl KVEntry for IndexeddbEntry {
  fn key(&self) -> &[u8] {
    self.key.as_ref()
  }

  fn value(&self) -> &[u8] {
    self.value.as_ref()
  }
}
