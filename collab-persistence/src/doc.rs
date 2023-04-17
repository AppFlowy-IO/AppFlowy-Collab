use std::fmt::Debug;

use sled::Iter;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

use crate::db::{batch_get, batch_remove};
use crate::keys::{
  doc_name_from_key, make_doc_end_key, make_doc_id_key, make_doc_start_key, make_doc_state_key,
  make_doc_update_key, make_state_vector_key, DocID, Key, DOC_SPACE, DOC_SPACE_OBJECT,
  DOC_SPACE_OBJECT_KEY,
};
use crate::{DbContext, PersistenceError};

pub struct YrsDocDB<'a> {
  pub(crate) uid: i64,
  pub(crate) context: &'a DbContext,
}

impl<'a> YrsDocDB<'a> {
  pub fn create_new_doc<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let doc_state = txn.encode_diff_v1(&StateVector::default());
    let sv = txn.state_vector().encode_v1();
    let doc_id = self.get_or_create_did(object_id.as_ref())?;
    tracing::trace!(
      "[doc:{}]:Create new doc {} for {:?}",
      doc_id,
      doc_id,
      object_id
    );
    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);
    self.context.db.write().insert(&doc_state_key, doc_state)?;
    self.context.db.write().insert(&sv_key, sv)?;
    Ok(())
  }

  pub fn is_exist<K: AsRef<[u8]> + ?Sized>(&self, doc_id: &K) -> bool {
    self.get_doc_id(doc_id).is_some()
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
  ) -> Result<(), PersistenceError> {
    if let Some(did) = self.get_doc_id(object_id) {
      let doc_state_key = make_doc_state_key(did);
      if let Some(doc_state) = self.context.db.read().get(doc_state_key)? {
        let update = Update::decode_v1(doc_state.as_ref())?;
        txn.apply_update(update);

        let update_start = make_doc_update_key(did, 0);
        let update_end = make_doc_update_key(did, DocID::MAX);
        tracing::trace!(
          "[doc:{}]:Get update from {:?} to {:?}",
          did,
          update_start.as_ref(),
          update_end.as_ref(),
        );
        let encoded_updates = batch_get(&self.context.db.read(), &update_start, &update_end)?;
        tracing::trace!(
          "{:?}: Number of encoded_updates: {}",
          object_id,
          encoded_updates.len()
        );
        for encoded_update in encoded_updates {
          let update = Update::decode_v1(encoded_update.as_ref())?;
          txn.apply_update(update);
        }
      }

      Ok(())
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }

  pub fn push_update<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let doc_id = self.get_or_create_did(object_id.as_ref())?;
    tracing::trace!("[doc:{}]:Insert update for {:?}", doc_id, object_id);
    self.context.insert_doc_update(doc_id, update.to_vec())?;
    Ok(())
  }

  ///
  ///
  /// # Arguments
  ///
  /// * `object_id`:
  ///
  /// returns: Result<(), PersistenceError>
  ///
  /// # Examples
  ///
  /// ```
  ///
  /// ```
  pub fn delete_doc<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
  ) -> Result<(), PersistenceError> {
    if let Some(did) = self.get_doc_id(object_id) {
      tracing::trace!("[{}] delete {:?} doc", did, object_id);
      let key = make_doc_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let mut db = self.context.db.write();
      let _ = db.remove(key);

      let start = make_doc_start_key(did);
      let end = make_doc_end_key(did);
      let _ = batch_remove(&mut db, start, end);

      let doc_state_key = make_doc_state_key(did);
      let sv_key = make_state_vector_key(did);
      let _ = db.remove(doc_state_key);
      let _ = db.remove(sv_key);
    }
    Ok(())
  }

  pub fn get_all_docs(&self) -> Result<DocsNameIter, PersistenceError> {
    let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
    let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);
    let iter = self.context.db.read().range(from..=to);

    Ok(DocsNameIter { iter })
  }

  pub fn get_updates<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<Vec<Update>, PersistenceError> {
    if let Some(doc_id) = self.get_doc_id(object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, DocID::MAX);
      let encoded_updates = batch_get(&self.context.db.read(), &start, &end)?;
      let mut updates = vec![];
      for encoded_update in encoded_updates {
        updates.push(Update::decode_v1(encoded_update.as_ref())?);
      }
      Ok(updates)
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }

  /// Get or create a document id for the given object id.
  fn get_or_create_did<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
  ) -> Result<DocID, PersistenceError> {
    if let Some(did) = self.get_doc_id(object_id.as_ref()) {
      Ok(did)
    } else {
      let key = make_doc_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let new_did = self.context.create_doc_id_for_key(key)?;
      Ok(new_did)
    }
  }

  fn get_doc_id<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Option<DocID> {
    let uid = &self.uid.to_be_bytes();
    let key = make_doc_id_key(uid, object_id.as_ref());
    self.context.get_doc_id_for_key(key)
  }
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
