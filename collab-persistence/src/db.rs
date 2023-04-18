use std::fmt::Debug;
use std::io::Write;
use std::ops::{Deref, RangeTo};
use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use sled::{Batch, Db, IVec};
use smallvec::{smallvec, SmallVec};

use crate::doc::YrsDocDB;
use crate::error::PersistenceError;
use crate::keys::DOC_KEY_SPACE;
use crate::keys::{
  clock_from_key, make_doc_update_key, make_doc_update_key_prefix, make_snapshot_update_key,
  make_snapshot_update_key_prefix, Clock, DocID, Key, SnapshotID, TERMINATOR,
};
use crate::snapshot::YrsSnapshotDB;

#[derive(Clone)]
pub struct CollabKV {
  pub(crate) db: Db,
  doc_context: Arc<DbContext>,
  snapshot_context: Arc<DbContext>,
}

impl CollabKV {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let db = sled::open(path)?;
    let doc_context = Arc::new(DbContext::new(db.clone()));
    let snapshot_context = Arc::new(DbContext::new(db.clone()));
    Ok(Self {
      db,
      doc_context,
      snapshot_context,
    })
  }

  pub fn doc(&self, uid: i64) -> YrsDocDB {
    YrsDocDB {
      uid,
      context: self.doc_context.as_ref(),
    }
  }

  pub fn snapshot(&self, uid: i64) -> YrsSnapshotDB {
    YrsSnapshotDB {
      context: self.snapshot_context.as_ref(),
      uid,
    }
  }
}

impl Deref for CollabKV {
  type Target = Db;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

pub struct DbContext {
  pub(crate) db: RwLock<Db>,
}

pub type OID = u64;
const OID_LEN: usize = 8;

impl DbContext {
  pub fn new(db: Db) -> Self {
    Self {
      db: RwLock::new(db),
    }
  }

  pub fn insert_doc_update<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    doc_id: DocID,
    object_id: &K,
    value: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    let db = self.db.write();
    let update_key = self.create_update_key(
      doc_id,
      &db,
      object_id,
      make_doc_update_key,
      make_doc_update_key_prefix,
    )?;
    let _ = db.insert(update_key, value)?;
    Ok(())
  }

  pub fn insert_snapshot_update<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    snapshot_id: SnapshotID,
    object_id: &K,
    value: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    let db = self.db.write();
    let update_key = self.create_update_key(
      snapshot_id,
      &db,
      object_id,
      make_snapshot_update_key,
      make_snapshot_update_key_prefix,
    )?;
    let _ = db.insert(update_key, value)?;
    Ok(())
  }

  pub fn create_doc_id_for_key(&self, key: Key<20>) -> Result<DocID, PersistenceError> {
    self.create_id_for_key(key)
  }

  pub fn get_doc_id_for_key(&self, key: Key<20>) -> Option<DocID> {
    self.get_id_for_key(key)
  }

  pub fn create_snapshot_id_for_key(&self, key: Key<20>) -> Result<SnapshotID, PersistenceError> {
    self.create_id_for_key(key)
  }

  pub fn get_snapshot_id_for_key(&self, key: Key<20>) -> Option<SnapshotID> {
    self.get_id_for_key(key)
  }

  fn get_id_for_key(&self, key: Key<20>) -> Option<OID> {
    let key_id = self.db.read().get(key).ok()??;
    let id_value = self.db.read().get(key_id.as_ref()).ok()??;
    // println!("get key:{:?}, value: {:?}", key_id.as_ref(), id_value);

    let mut bytes = [0; OID_LEN];
    bytes[0..OID_LEN].copy_from_slice(id_value.as_ref());
    Some(OID::from_be_bytes(bytes))
  }

  pub fn create_id_for_key(&self, key: Key<20>) -> Result<OID, PersistenceError> {
    let db = self.db.write();
    let new_id = match self.last_id(&db) {
      None => 0,
      Some(last_id) => last_id + 1,
    };

    let new_key = gen_new_key(&db);
    db.insert(key, new_key.as_ref())?;

    let _ = db.insert(new_key.as_ref(), &new_id.to_be_bytes())?;
    drop(db);
    Ok(new_id)
  }

  fn create_update_key<F1, F2, K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    id: OID,
    db: &Db,
    object_id: &K,
    make_update_key: F1,
    make_update_key_prefix: F2,
  ) -> Result<Key<16>, PersistenceError>
  where
    F1: Fn(OID, Clock) -> Key<16>,
    F2: Fn(OID) -> Key<12>,
  {
    let last_clock = {
      // let start = make_update_key(id, OID::MIN);
      let start = make_update_key_prefix(id);
      if let Some(Ok((k, _v))) = db.scan_prefix(start) // Create a range up to (excluding) the given key
        .last()
      {
        let last_clock = clock_from_key(k.as_ref());
        Clock::from_be_bytes(last_clock.try_into().unwrap())
      } else {
        0
      }
    };
    let clock = last_clock + 1;
    let new_key = make_update_key(id, clock);
    tracing::debug!(
      "[{}-{:?}]: create new update key {:?}",
      id,
      object_id,
      new_key.as_ref()
    );
    Ok(new_key)
  }

  fn last_id(&self, db: &Db) -> Option<OID> {
    let given_key: &[u8; 2] = &[0, 1];
    let (_, v) = db
        .range::<&[u8;2],RangeTo<&[u8;2]>>(..given_key) // Create a range up to (excluding) the given key
        .next_back()?.ok()?;
    // let (_, v) = db.scan_prefix(self.max_key.as_ref()).next_back()?.ok()?;
    Some(OID::from_be_bytes(v.as_ref().try_into().ok()?))
  }
}

pub(crate) fn batch_get<K: AsRef<[u8]>>(
  db: &Db,
  from_key: K,
  to_key: K,
) -> Result<Vec<IVec>, PersistenceError> {
  let iter = db.range(from_key..=to_key);
  let mut items = vec![];
  for item in iter {
    let (_, value) = item?;
    items.push(value)
  }
  Ok(items)
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

pub(crate) fn batch_remove<K: AsRef<[u8]>>(
  db: &mut Db,
  from_key: K,
  to_key: K,
) -> Result<(), PersistenceError> {
  let mut batch = Batch::default();
  let iter = db.range(from_key..=to_key);
  for key in iter {
    let key = key?.0;
    batch.remove(key);
  }
  db.apply_batch(batch)?;
  Ok(())
}

fn gen_new_key(db: &Db) -> Key<10> {
  let key_value = db.generate_id().unwrap();
  let mut v: SmallVec<[u8; 10]> = smallvec![DOC_KEY_SPACE, DOC_KEY_SPACE];
  v.write_all(&key_value.to_be_bytes()).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn make_update_key_prefix(prefix: &[u8], oid: OID) -> Key<12> {
  let mut v: SmallVec<[u8; 12]> = SmallVec::from(prefix);
  v.write_all(&oid.to_be_bytes()).unwrap();
  Key(v)
}
