use std::fmt::Debug;
use std::io::Write;
use std::ops::Deref;

use parking_lot::RwLock;
use sled::{Batch, Db};
use smallvec::SmallVec;
use std::sync::Arc;

use crate::error::PersistenceError;
use crate::keys::{
  clock_from_key, make_doc_update_key, make_snapshot_update_key, Clock, DocID, Key, SnapshotID,
};

use crate::kv::{KVEntry, KVStore};
use crate::oid::{OID, OID_GEN, OID_LEN};

#[derive(Clone)]
pub struct CollabDB<S> {
  pub(crate) store: S,
  pub doc_store: Arc<SubStore<S>>,
  pub snapshot_store: Arc<SubStore<S>>,
}

impl<S> CollabDB<S>
where
  S: KVStore<'static> + Clone,
{
  pub fn new(store: S) -> Result<Self, PersistenceError> {
    let doc_store = Arc::new(SubStore::new(store.clone()));
    let snapshot_store = Arc::new(SubStore::new(store.clone()));
    Ok(Self {
      store,
      doc_store,
      snapshot_store,
    })
  }
}

impl<S> Deref for CollabDB<S> {
  type Target = S;

  fn deref(&self) -> &Self::Target {
    &self.store
  }
}

pub struct SubStore<T>(RwLock<T>);

impl<T> SubStore<T>
where
  T: KVStore<'static>,
{
  pub fn new(db: T) -> Self {
    Self(RwLock::new(db))
  }
}

impl<T> Deref for SubStore<T>
where
  T: KVStore<'static>,
{
  type Target = RwLock<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub fn insert_snapshot_update<'a, K, S>(
  store: &S,
  snapshot_id: SnapshotID,
  object_id: &K,
  value: Vec<u8>,
) -> Result<(), PersistenceError>
where
  K: AsRef<[u8]> + ?Sized + Debug,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let update_key = create_update_key(snapshot_id, store, object_id, make_snapshot_update_key)?;
  let _ = store.insert(update_key, value)?;
  Ok(())
}

pub fn insert_doc_update<'a, K, S>(
  db: &S,
  doc_id: DocID,
  object_id: &K,
  value: Vec<u8>,
) -> Result<(), PersistenceError>
where
  K: AsRef<[u8]> + ?Sized + Debug,
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let update_key = create_update_key(doc_id, db, object_id, make_doc_update_key)?;
  let _ = db.insert(update_key, value)?;
  Ok(())
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
  let max_key = make_update_key(id, Clock::MAX);
  let last_clock = if let Ok(Some(entry)) = store.next_back_entry(max_key.as_ref()) {
    let clock_byte = clock_from_key(entry.key());
    Clock::from_be_bytes(clock_byte.try_into().unwrap())
  } else {
    0
  };

  let clock = last_clock + 1;
  let new_key = make_update_key(id, clock);
  tracing::debug!(
    "🤲collab => [{}-{:?}]: New update key {:?}",
    id,
    object_id,
    new_key.as_ref()
  );
  Ok(new_key)
}

pub fn get_id_for_key<'a, S>(store: &S, key: Key<20>) -> Option<DocID>
where
  S: KVStore<'a>,
{
  let value = store.get(key.as_ref()).ok()??;
  let mut bytes = [0; OID_LEN];
  bytes[0..OID_LEN].copy_from_slice(value.as_ref());
  Some(OID::from_be_bytes(bytes))
}

pub fn create_id_for_key<'a, S>(store: &S, key: Key<20>) -> Result<DocID, PersistenceError>
where
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let new_id = OID_GEN.lock().next_id();
  let _ = store.insert(key.as_ref(), &new_id.to_be_bytes())?;
  Ok(new_id)
}

#[allow(dead_code)]
pub(crate) fn batch_insert<'a, K: AsRef<[u8]>>(
  db: &mut Db,
  items: impl IntoIterator<Item = (K, &'a [u8])>,
) -> Result<(), PersistenceError> {
  let mut batch = Batch::default();
  let items = items.into_iter();
  items.for_each(|(key, value)| {
    batch.insert(key.as_ref(), value);
  });
  db.apply_batch(batch)?;
  Ok(())
}

pub fn make_update_key_prefix(prefix: &[u8], oid: OID) -> Key<12> {
  let mut v: SmallVec<[u8; 12]> = SmallVec::from(prefix);
  v.write_all(&oid.to_be_bytes()).unwrap();
  Key(v)
}
