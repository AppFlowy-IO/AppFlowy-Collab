use std::fmt::Debug;
use std::io::Write;
use std::ops::RangeBounds;
use std::sync::Arc;

use crate::local_storage::kv::PersistenceError;
use crate::local_storage::kv::keys::*;
use crate::local_storage::kv::oid::{DocIDGen, OID};
use crate::local_storage::kv::snapshot::CollabSnapshot;
use smallvec::SmallVec;
use yrs::{TransactionMut, Update};

pub trait KVTransactionDB: Send + Sync + 'static {
  type TransactionAction<'a>;

  fn read_txn<'a, 'b>(&'b self) -> Self::TransactionAction<'a>
  where
    'b: 'a;

  fn write_txn<'a, 'b>(&'b self) -> Self::TransactionAction<'a>
  where
    'b: 'a;

  fn with_write_txn<'a, 'b, Output>(
    &'b self,
    f: impl FnOnce(&Self::TransactionAction<'a>) -> Result<Output, PersistenceError>,
  ) -> Result<Output, PersistenceError>
  where
    'b: 'a;

  fn flush(&self) -> Result<(), PersistenceError>;
}

pub trait KVStore<'a> {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Value: AsRef<[u8]>;
  type Error: Into<PersistenceError> + Debug;

  /// Get a value by key
  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error>;

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error>;

  /// Remove a key, returning the last value if it exists
  fn remove(&self, key: &[u8]) -> Result<(), Self::Error>;

  /// Remove all keys in the range [from..to]
  /// The upper bound itself is not included on the iteration result.
  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error>;

  /// Return an iterator over the range of keys
  /// The upper bound itself is not included on the iteration result.
  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error>;

  /// Return the entry prior to the given key
  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error>;
}

impl<T> KVStore<'static> for Arc<T>
where
  T: KVStore<'static>,
{
  type Range = <T as KVStore<'static>>::Range;
  type Entry = <T as KVStore<'static>>::Entry;
  type Value = <T as KVStore<'static>>::Value;
  type Error = <T as KVStore<'static>>::Error;

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    (**self).get(key)
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    (**self).insert(key, value)
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    (**self).remove(key)
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    (**self).remove_range(from, to)
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    self.as_ref().range(range)
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    (**self).next_back_entry(key)
  }
}

pub fn insert_snapshot_update<'a, K, S>(
  store: &S,
  snapshot_id: SnapshotID,
  object_id: &K,
  data: Vec<u8>,
) -> Result<(), PersistenceError>
where
  K: AsRef<[u8]> + ?Sized + Debug,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let snapshot = CollabSnapshot::new(data).to_vec();
  let update_key = create_update_key(snapshot_id, store, object_id, make_snapshot_update_key)?;
  store.insert(update_key, snapshot)?;
  Ok(())
}

pub fn insert_doc_update<'a, K, S>(
  db: &S,
  doc_id: DocID,
  object_id: &K,
  value: Vec<u8>,
) -> Result<Vec<u8>, PersistenceError>
where
  K: AsRef<[u8]> + ?Sized + Debug,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let update_key = create_update_key(doc_id, db, object_id, make_doc_update_key)?;
  if let Ok(Some(_)) = db.get(update_key.as_ref()) {
    // The duplicate key might corrupt the document data when restoring from the disk,
    // So we return an error here.
    return Err(PersistenceError::DuplicateUpdateKey);
  }
  db.insert(update_key.as_ref(), value)?;
  Ok(update_key.to_vec())
}

pub fn get_last_update_key<'a, S, F>(
  store: &S,
  id: OID,
  make_update_key: F,
) -> Result<Key<16>, PersistenceError>
where
  F: Fn(OID, Clock) -> Key<16>,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let last_clock = get_last_update_clock(store, id, &make_update_key)?;
  Ok(make_update_key(id, last_clock))
}

fn create_update_key<'a, F, K, S>(
  id: OID,
  store: &S,
  _object_id: &K,
  make_update_key: F,
) -> Result<Key<16>, PersistenceError>
where
  F: Fn(OID, Clock) -> Key<16>,
  K: AsRef<[u8]> + ?Sized + Debug,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let last_clock = get_last_update_clock(store, id, &make_update_key)?;
  let clock = last_clock + 1;
  let new_key = make_update_key(id, clock);
  Ok(new_key)
}

#[inline(always)]
fn get_last_update_clock<'a, S, F>(
  store: &S,
  id: OID,
  make_update_key: &F,
) -> Result<Clock, PersistenceError>
where
  F: Fn(OID, Clock) -> Key<16>,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let max_key = make_update_key(id, Clock::MAX);
  if let Ok(Some(entry)) = store.next_back_entry(max_key.as_ref()) {
    let clock_byte = clock_from_key(entry.key());
    Ok(Clock::from_be_bytes(clock_byte.try_into().unwrap()))
  } else {
    Ok(0)
  }
}

pub fn get_id_for_key<'a, S>(store: &S, key: Key<20>) -> Option<DocID>
where
  S: KVStore<'a>,
{
  let value = store.get(key.as_ref()).ok()??;
  let mut bytes = [0; DOC_ID_LEN];
  bytes[0..DOC_ID_LEN].copy_from_slice(value.as_ref());
  Some(OID::from_be_bytes(bytes))
}

pub fn insert_doc_id_for_key<'a, S>(store: &S, key: Key<20>) -> Result<DocID, PersistenceError>
where
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let new_id = DocIDGen::next_id();
  store.insert(key.as_ref(), new_id.to_be_bytes())?;
  Ok(new_id)
}

pub fn make_update_key_prefix(prefix: &[u8], oid: OID) -> Key<12> {
  let mut v: SmallVec<[u8; 12]> = SmallVec::from(prefix);
  v.write_all(&oid.to_be_bytes()).unwrap();
  Key(v)
}

// Extension trait for `TransactionMut`
pub trait TransactionMutExt<'doc> {
  /// Applies an update to the document. If the update is invalid, it will return an error.
  /// It allows to catch panics from `apply_update`.
  fn try_apply_update(&mut self, update: Update) -> Result<(), PersistenceError>;
}

impl<'doc> TransactionMutExt<'doc> for TransactionMut<'doc> {
  fn try_apply_update(&mut self, update: Update) -> Result<(), PersistenceError> {
    self.apply_update(update)?;
    Ok(())
  }
}

/// This trait is used to represents as the generic Range of different implementation.
pub trait KVRange<'a> {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Error: Into<PersistenceError>;

  fn kv_range(self) -> Result<Self::Range, Self::Error>;
}

/// A key-value entry
pub trait KVEntry {
  fn key(&self) -> &[u8];
  fn value(&self) -> &[u8];
}
