use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use sled::{Batch, Db, IVec};

use parking_lot::RwLock;

use crate::doc::YrsDocDB;
use crate::error::PersistenceError;
use crate::keys::{
  clock_from_key, make_doc_update_key, make_snapshot_key, DocID, Key, SnapshotID, DOC_SPACE,
  DOC_SPACE_OBJECT_KEY, SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT,
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
    let doc_context = Arc::new(DbContext::new(
      [DOC_SPACE, DOC_SPACE_OBJECT_KEY].as_ref(),
      db.clone(),
    ));
    let snapshot_context = Arc::new(DbContext::new(
      [SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT].as_ref(),
      db.clone(),
    ));
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
  max_key: Vec<u8>,
  pub(crate) db: RwLock<Db>,
}

impl DbContext {
  pub fn new(max_key: &[u8], db: Db) -> Self {
    Self {
      max_key: max_key.to_vec(),
      db: RwLock::new(db),
    }
  }

  pub fn insert_doc_update(&self, doc_id: DocID, value: Vec<u8>) -> Result<(), PersistenceError> {
    let db = self.db.write();
    let update_key = self.new_update_key(doc_id, &db, make_doc_update_key)?;
    let _ = db.insert(update_key, value)?;
    Ok(())
  }

  pub fn insert_snapshot_update(
    &self,
    snapshot_id: SnapshotID,
    value: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    let db = self.db.write();
    let update_key = self.new_update_key(snapshot_id, &db, make_snapshot_key)?;
    let _ = db.insert(update_key, value)?;
    Ok(())
  }

  pub fn create_doc_id_for_key(&self, key: Key<20>) -> Result<DocID, PersistenceError> {
    self.create_id_for_key(key)
  }

  pub fn create_snapshot_id_for_key(&self, key: Key<20>) -> Result<DocID, PersistenceError> {
    self.create_id_for_key(key)
  }

  pub fn create_id_for_key(&self, key: Key<20>) -> Result<DocID, PersistenceError> {
    let db = self.db.write();
    let last_doc_id = self.last_doc_id(&db).unwrap_or(0);
    let new_doc_id = last_doc_id + 1;
    let _ = db.insert(key, &new_doc_id.to_be_bytes())?;
    drop(db);
    Ok(new_doc_id)
  }

  fn new_update_key<F>(
    &self,
    doc_id: DocID,
    db: &Db,
    make_update_key: F,
  ) -> Result<Key<12>, PersistenceError>
  where
    F: Fn(u32, u32) -> Key<12>,
  {
    let last_clock = {
      let end = make_update_key(doc_id, u32::MAX);
      if let Some((k, _v)) = db.get_lt(&end)? {
        let last_key = k.as_ref();
        let last_clock = clock_from_key(last_key);
        u32::from_be_bytes(last_clock.try_into().unwrap())
      } else {
        0
      }
    };
    let clock = last_clock + 1;
    tracing::trace!("[{}] create new update key {}", doc_id, clock);
    Ok(make_update_key(doc_id, clock))
  }

  fn last_doc_id(&self, db: &Db) -> Option<DocID> {
    let key: &[u8] = self.max_key.as_ref();
    let (_k, v) = db.get_lt(key).ok()??;
    Some(DocID::from_be_bytes(v.as_ref().try_into().ok()?))
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
