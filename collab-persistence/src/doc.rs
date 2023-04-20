use std::fmt::Debug;

use sled::Iter;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

use crate::keys::{
  doc_name_from_key, make_doc_end_key, make_doc_id_key, make_doc_start_key, make_doc_state_key,
  make_doc_update_key, make_state_vector_key, Clock, DocID, Key, DOC_SPACE, DOC_SPACE_OBJECT,
  DOC_SPACE_OBJECT_KEY,
};
use crate::kv::KVEntry;
use crate::kv::KV;
use crate::{create_id_for_key, get_id_for_key, insert_doc_update, PersistenceError, SubStore};

pub struct YrsDocDB<'a, S> {
  pub(crate) uid: i64,
  pub(crate) store: &'a SubStore<S>,
}

impl<'a, S> YrsDocDB<'a, S>
where
  S: KV,
  PersistenceError: From<<S as KV>::Error>,
{
  pub fn create_new_doc<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    if self.is_exist(object_id) {
      tracing::warn!("🟡{:?} already exist", object_id);
    }
    let store = self.store.write();
    let doc_id = get_or_create_did(self.uid, &*store, object_id.as_ref())?;
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
    store.insert(doc_state_key, doc_state)?;
    store.insert(sv_key, sv)?;

    Ok(())
  }

  pub fn flush_doc<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    let doc_id = get_or_create_did(self.uid, &*store, object_id)?;
    tracing::trace!("🤲collab => [{}-{:?}]: flush doc", doc_id, object_id);

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
    // Insert new doc state and state vector
    store.insert(doc_state_key, doc_state)?;
    store.insert(sv_key, sv)?;

    // Remove the updates
    let start = make_doc_start_key(doc_id);
    let end = make_doc_end_key(doc_id);
    store.remove_range(start.as_ref(), end.as_ref())?;

    Ok(())
  }

  pub fn is_exist<K: AsRef<[u8]> + ?Sized + Debug>(&self, object_id: &K) -> bool {
    let doc_id = get_doc_id(self.uid, &*self.store.read(), object_id);
    match doc_id {
      None => false,
      Some(_) => true,
    }
  }

  pub fn load_doc<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
    txn: &mut TransactionMut,
  ) -> Result<u32, PersistenceError> {
    let mut update_count = 0;
    let store = self.store.read();

    if let Some(doc_id) = get_doc_id(self.uid, &*store, object_id) {
      tracing::trace!("🤲collab => [{}-{:?}]: load doc", doc_id, object_id);
      let doc_state_key = make_doc_state_key(doc_id);
      if let Some(doc_state) = store.get(doc_state_key.as_ref())? {
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

        let encoded_updates = store.iter_range(update_start.as_ref(), update_end.as_ref())?;
        for encoded_update in encoded_updates {
          update_count += 1;
          let update = Update::decode_v1(encoded_update.value())?;
          txn.apply_update(update);
        }
      } else {
        tracing::error!(
          "🔴collab => [{}-{:?}]: the doc state should not be empty",
          doc_id,
          object_id
        );
      }
      tracing::debug!(
        "[{:?}-{:?}]: num of updates: {}",
        doc_id,
        object_id,
        update_count,
      );

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
    let store = self.store.read();
    match get_doc_id(self.uid, &*store, object_id.as_ref()) {
      None => {
        tracing::error!(
          "🔴Insert update failed. Can't find the doc for {:?}",
          object_id
        );
      },
      Some(doc_id) => {
        insert_doc_update(&*store, doc_id, object_id, update.to_vec())?;
      },
    }
    Ok(())
  }

  pub fn delete_doc<K: AsRef<[u8]> + ?Sized + Debug>(
    &self,
    object_id: &K,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    if let Some(did) = get_doc_id(self.uid, &*store, object_id) {
      tracing::trace!("🤲collab => [{}] delete {:?} doc", did, object_id);
      let key = make_doc_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let _ = store.remove(key.as_ref());

      let start = make_doc_start_key(did);
      let end = make_doc_end_key(did);
      store.remove_range(start.as_ref(), end.as_ref())?;

      let doc_state_key = make_doc_state_key(did);
      let sv_key = make_state_vector_key(did);
      let _ = store.remove(doc_state_key.as_ref());
      let _ = store.remove(sv_key.as_ref());
    }
    Ok(())
  }

  pub fn get_all_docs(
    &self,
  ) -> Result<NameIter<<S as KV>::Range, <S as KV>::Entry>, PersistenceError> {
    let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
    let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);
    let iter = self.store.read().iter_range(from.as_ref(), to.as_ref())?;
    Ok(NameIter { iter })
  }

  pub fn get_updates<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<Vec<Update>, PersistenceError> {
    let store = self.store.read();
    if let Some(doc_id) = get_doc_id(self.uid, &*store, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);

      let encoded_updates = store.iter_range(start.as_ref(), end.as_ref())?;
      let mut updates = vec![];
      for encoded_update in encoded_updates {
        updates.push(Update::decode_v1(encoded_update.value())?);
      }

      Ok(updates)
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }
}

/// Get or create a document id for the given object id.
fn get_or_create_did<K, S>(uid: i64, store: &S, object_id: &K) -> Result<DocID, PersistenceError>
where
  S: KV,
  PersistenceError: From<<S as KV>::Error>,
  K: AsRef<[u8]> + ?Sized + Debug,
{
  if let Some(did) = get_doc_id(uid, store, object_id.as_ref()) {
    Ok(did)
  } else {
    let key = make_doc_id_key(&uid.to_be_bytes(), object_id.as_ref());
    let new_did = create_id_for_key(store, key)?;
    Ok(new_did)
  }
}

fn get_doc_id<K, S>(uid: i64, store: &S, object_id: &K) -> Option<DocID>
where
  S: KV,
  K: AsRef<[u8]> + ?Sized,
{
  let uid = &uid.to_be_bytes();
  let key = make_doc_id_key(uid, object_id.as_ref());
  get_id_for_key(store, key)
}

pub struct NameIter<I, E>
where
  I: Iterator<Item = E>,
  E: KVEntry,
{
  iter: I,
}

impl<I, E> Iterator for NameIter<I, E>
where
  I: Iterator<Item = E>,
  E: KVEntry,
{
  type Item = String;

  fn next(&mut self) -> Option<Self::Item> {
    let entry = self.iter.next()?;
    let content = doc_name_from_key(entry.key());
    Some(String::from_utf8_lossy(content).to_string())
  }
}
