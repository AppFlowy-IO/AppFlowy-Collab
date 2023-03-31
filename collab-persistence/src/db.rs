use crate::doc::YrsDoc;
use crate::error::PersistenceError;
use crate::snapshot::YrsSnapshot;
use sled::{Batch, Db, IVec};
use std::ops::Deref;
use std::path::Path;

#[derive(Clone)]
pub struct CollabKV {
  pub(crate) db: Db,
}

impl CollabKV {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let db = sled::open(path)?;
    Ok(Self { db })
  }

  pub fn doc(&self, uid: i64) -> YrsDoc {
    YrsDoc { db: self, uid }
  }

  pub fn snapshot(&self, uid: i64) -> YrsSnapshot {
    YrsSnapshot { db: self, uid }
  }

  pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<IVec>, PersistenceError> {
    let value = self.db.get(key)?;
    Ok(value)
  }

  pub fn insert<K: AsRef<[u8]>>(&self, key: K, value: &[u8]) -> Result<(), PersistenceError> {
    let _ = self.db.insert(key, value)?;
    Ok(())
  }

  pub fn batch_get<K: AsRef<[u8]>>(
    &self,
    from_key: K,
    to_key: K,
  ) -> Result<Vec<IVec>, PersistenceError> {
    let iter = self.db.range(from_key..=to_key);
    let mut items = vec![];
    for item in iter {
      let (_, value) = item?;
      items.push(value)
    }
    Ok(items)
  }

  pub fn batch_insert<'a, K: AsRef<[u8]>>(
    &self,
    items: impl IntoIterator<Item = (K, &'a [u8])>,
  ) -> Result<(), PersistenceError> {
    let mut batch = Batch::default();
    let items = items.into_iter();
    items.for_each(|(key, value)| {
      batch.insert(key.as_ref(), value);
    });
    self.db.apply_batch(batch)?;
    Ok(())
  }

  pub fn batch_remove<K: AsRef<[u8]>>(
    &self,
    from_key: K,
    to_key: K,
  ) -> Result<(), PersistenceError> {
    let mut batch = Batch::default();
    let iter = self.db.range(from_key..=to_key);
    for key in iter {
      let key = key?.0;
      batch.remove(key);
    }
    self.db.apply_batch(batch)?;
    Ok(())
  }
}

impl Deref for CollabKV {
  type Target = Db;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}
