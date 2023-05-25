use std::fmt::Debug;
use std::io::Write;
use std::ops::Deref;
use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use parking_lot::RwLock;
use smallvec::SmallVec;
use yrs::{TransactionMut, Update};

use crate::error::PersistenceError;
use crate::keys::{
  clock_from_key, make_doc_update_key, make_snapshot_update_key, Clock, DocID, Key, SnapshotID,
};
use crate::kv::{KVEntry, KVStore};
use crate::oid::{DOC_ID_LEN, LOCAL_DOC_ID_GEN, OID};
use crate::snapshot::CollabSnapshot;

#[derive(Clone)]
pub struct CollabDB<S> {
  pub store: Arc<RwStore<S>>,
}

impl<S> CollabDB<S>
where
  S: KVStore<'static> + Clone,
{
  pub fn new(store: S) -> Result<Self, PersistenceError> {
    let store = Arc::new(RwStore::new(store));
    Ok(Self { store })
  }
}

impl<S> Deref for CollabDB<S> {
  type Target = Arc<RwStore<S>>;

  fn deref(&self) -> &Self::Target {
    &self.store
  }
}

pub struct RwStore<T>(RwLock<T>);

impl<T> RwStore<T>
where
  T: KVStore<'static>,
{
  pub fn new(db: T) -> Self {
    Self(RwLock::new(db))
  }
}

impl<T> Deref for RwStore<T>
where
  T: KVStore<'static>,
{
  type Target = RwLock<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub fn insert_snapshot_update<'a, K1, K2, S>(
  store: &S,
  update_key: K2,
  snapshot_id: SnapshotID,
  object_id: &K1,
  data: Vec<u8>,
) -> Result<(), PersistenceError>
where
  K1: AsRef<[u8]> + ?Sized + Debug,
  K2: Into<Vec<u8>>,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let snapshot = CollabSnapshot::new(data, update_key.into()).to_vec();
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
  object_id: &K,
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
  tracing::trace!(
    "[🦀Collab] => [{}-{:?}]: new update {:?}",
    id,
    object_id,
    new_key.as_ref()
  );
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

pub fn make_doc_id_for_key<'a, S>(store: &S, key: Key<20>) -> Result<DocID, PersistenceError>
where
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let new_id = LOCAL_DOC_ID_GEN.lock().next_id();
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
    match panic::catch_unwind(AssertUnwindSafe(|| {
      self.apply_update(update);
    })) {
      Ok(_) => Ok(()),
      Err(_) => Err(PersistenceError::InternalError),
    }
  }
}
