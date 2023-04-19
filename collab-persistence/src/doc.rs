use std::fmt::Debug;

use sled::transaction::{ConflictableTransactionError, TransactionError};
use sled::{Batch, Db, Iter};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

use crate::db::{batch_get, batch_remove};
use crate::keys::{
  doc_name_from_key, make_doc_end_key, make_doc_id_key, make_doc_start_key, make_doc_state_key,
  make_doc_update_key, make_state_vector_key, Clock, DocID, Key, DOC_SPACE, DOC_SPACE_OBJECT,
  DOC_SPACE_OBJECT_KEY,
};
use crate::{
  create_doc_id_for_key, get_doc_id_for_key, insert_doc_update, KVStore, PersistenceError,
};

pub struct YrsDocDB<'a> {
  pub(crate) uid: i64,
  pub(crate) store: &'a KVStore,
}

impl<'a> YrsDocDB<'a> {
  pub fn create_new_doc<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    let doc_id = get_or_create_did(self.uid, &store, object_id.as_ref())?;
    match store
      .transaction(|db| {
        tracing::trace!(
          "🤲collab => [{}:{:?}]: New doc:{}",
          doc_id,
          object_id,
          doc_id
        );
        let doc_state = txn.encode_diff_v1(&StateVector::default());
        let sv = txn.state_vector().encode_v1();
        let doc_state_key = make_doc_state_key(doc_id);
        let sv_key = make_state_vector_key(doc_id);

        tracing::trace!(
          "🤲collab => [{}-{:?}] insert doc state: {:?}",
          doc_id,
          object_id,
          doc_state_key
        );
        db.insert(doc_state_key.as_ref(), doc_state)?;
        db.insert(sv_key.as_ref(), sv)?;
        Ok::<(), ConflictableTransactionError<PersistenceError>>(())
      })
      .map_err(|_: TransactionError<PersistenceError>| PersistenceError::InternalError)
    {
      Ok(_) => {},
      Err(e) => {
        tracing::error!("🔴collab => create doc failed. error: {:?}", e);
      },
    }

    Ok(())
  }

  pub fn flush_doc<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    let doc_id = get_or_create_did(self.uid, &store, object_id)?;
    tracing::trace!("🤲collab => [{}-{:?}]: flush doc", doc_id, object_id);
    match store
      .transaction(|db| {
        let doc_state = txn.encode_state_as_update_v1(&StateVector::default());
        let sv = txn.state_vector().encode_v1();

        let doc_state_key = make_doc_state_key(doc_id);
        let sv_key = make_state_vector_key(doc_id);

        tracing::trace!(
          "🤲collab => [{}-{:?}] insert doc state: {:?} : {}",
          doc_id,
          object_id,
          doc_state_key.as_ref(),
          doc_state.len(),
        );
        db.insert(doc_state_key.as_ref(), doc_state)?;
        db.insert(sv_key.as_ref(), sv)?;
        db.flush();

        Ok::<(), ConflictableTransactionError<PersistenceError>>(())
      })
      .map_err(|_: TransactionError<PersistenceError>| PersistenceError::InternalError)
    {
      Ok(_) => {
        let start = make_doc_start_key(doc_id);
        let end = make_doc_end_key(doc_id);
        let mut batch = Batch::default();

        let iter = store.range(start..=end);
        for key in iter {
          let key = key?.0;
          batch.remove(key);
        }
        store.apply_batch(batch)?;
      },
      Err(e) => {
        tracing::error!("🔴collab => flush doc failed. error: {:?}", e);
      },
    }

    Ok(())
  }

  pub fn is_exist<K: AsRef<[u8]> + ?Sized + Debug>(&self, object_id: &K) -> bool {
    let doc_id = get_doc_id(self.uid, &self.store.read(), object_id);
    match doc_id {
      None => {
        tracing::trace!("🤲collab => {:?} not exist", object_id);
        false
      },
      Some(_) => true,
    }
  }

  ///
  ///
  /// # Arguments
  ///
  /// * `object_id`:
  /// * `txn`:
  ///
  /// returns: Result<(), PersistenceError>
  ///
  /// # Examples
  ///
  /// ```
  ///
  /// ```
  pub fn load_doc<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
    txn: &mut TransactionMut,
  ) -> Result<u32, PersistenceError> {
    let mut update_count = 0;
    let store = self.store.read();

    if let Some(doc_id) = get_doc_id(self.uid, &store, object_id) {
      tracing::trace!("🤲collab => [{}-{:?}]: load doc", doc_id, object_id);
      let doc_state_key = make_doc_state_key(doc_id);
      if let Some(doc_state) = store.get(doc_state_key)? {
        let update = Update::decode_v1(doc_state.as_ref())?;
        txn.apply_update(update);

        let update_start = make_doc_update_key(doc_id, 0);
        let update_end = make_doc_update_key(doc_id, Clock::MAX);
        tracing::trace!(
          "🤲collab => [{}-{:?}]: Get update from {:?} to {:?}",
          doc_id,
          object_id,
          update_start.as_ref(),
          update_end.as_ref(),
        );
        let encoded_updates = batch_get(&store, &update_start, &update_end)?;
        tracing::debug!(
          "[{:?}-{:?}]: num of updates: {}",
          doc_id,
          object_id,
          encoded_updates.len()
        );
        for encoded_update in encoded_updates {
          update_count += 1;
          let update = Update::decode_v1(encoded_update.as_ref())?;
          txn.apply_update(update);
        }
      } else {
        tracing::error!(
          "🔴collab => [{}-{:?}]: the doc state should not be empty",
          doc_id,
          object_id
        );
      }

      Ok(update_count)
    } else {
      tracing::trace!("🤲collab => {:?} not exist", object_id);
      Err(PersistenceError::DocumentNotExist)
    }
  }

  pub fn push_update<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    let doc_id = get_or_create_did(self.uid, &store, object_id.as_ref())?;
    insert_doc_update(&store, doc_id, object_id, update.to_vec())?;
    Ok(())
  }

  pub fn delete_doc<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    if let Some(did) = get_doc_id(self.uid, &store, object_id) {
      tracing::trace!("🤲collab => [{}] delete {:?} doc", did, object_id);
      let key = make_doc_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let _ = store.remove(key);

      let start = make_doc_start_key(did);
      let end = make_doc_end_key(did);
      let _ = batch_remove(&store, start, end);

      let doc_state_key = make_doc_state_key(did);
      let sv_key = make_state_vector_key(did);
      let _ = store.remove(doc_state_key);
      let _ = store.remove(sv_key);
    }
    Ok(())
  }

  pub fn get_all_docs(&self) -> Result<DocsNameIter, PersistenceError> {
    let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
    let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);
    let iter = self.store.read().range(from..=to);

    Ok(DocsNameIter { iter })
  }

  pub fn get_updates<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<Vec<Update>, PersistenceError> {
    let store = self.store.read();
    if let Some(doc_id) = get_doc_id(self.uid, &store, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);
      let encoded_updates = batch_get(&store, &start, &end)?;
      let mut updates = vec![];
      for encoded_update in encoded_updates {
        updates.push(Update::decode_v1(encoded_update.as_ref())?);
      }
      Ok(updates)
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }
}

/// Get or create a document id for the given object id.
fn get_or_create_did<K: AsRef<[u8]> + ?Sized + Debug>(
  uid: i64,
  db: &Db,
  object_id: &K,
) -> Result<DocID, PersistenceError> {
  if let Some(did) = get_doc_id(uid, db, object_id.as_ref()) {
    Ok(did)
  } else {
    let key = make_doc_id_key(&uid.to_be_bytes(), object_id.as_ref());
    let new_did = create_doc_id_for_key(db, key)?;
    Ok(new_did)
  }
}

fn get_doc_id<K: AsRef<[u8]> + ?Sized>(uid: i64, db: &Db, object_id: &K) -> Option<DocID> {
  let uid = &uid.to_be_bytes();
  let key = make_doc_id_key(uid, object_id.as_ref());
  get_doc_id_for_key(&db, key)
}

pub type DocName = String;

pub struct DocsNameIter {
  iter: Iter,
}

impl DocsNameIter {}

impl Iterator for DocsNameIter {
  type Item = DocName;

  fn next(&mut self) -> Option<Self::Item> {
    let (k, _) = self.iter.next()?.ok()?;
    let content = doc_name_from_key(k.as_ref());
    Some(String::from_utf8_lossy(content).to_string())
  }
}
