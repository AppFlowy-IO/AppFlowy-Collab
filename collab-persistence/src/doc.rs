use crate::keys::{
  doc_name_from_key, make_doc_end_key, make_doc_id, make_doc_start_key, make_doc_state_key,
  make_state_vector_key, make_update_key, DocID, Key, DID_SPACE, DOC_SPACE, SPACE,
};
use crate::{CollabKV, PersistenceError};
use sled::{IVec, Iter};

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

pub struct YrsDoc<'a> {
  pub(crate) db: &'a CollabKV,
}

impl<'a> YrsDoc<'a> {
  pub fn insert_or_create_new_doc<K: AsRef<[u8]> + ?Sized, T: ReadTxn>(
    &self,
    name: &K,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let doc_state = txn.encode_diff_v1(&StateVector::default());
    let sv = txn.state_vector().encode_v1();
    let did = self.get_or_create_did(name.as_ref())?;
    let doc_state_key = make_doc_state_key(did);
    let sv_key = make_state_vector_key(did);
    self.db.insert(&doc_state_key, &doc_state)?;
    self.db.insert(&sv_key, &sv)?;
    Ok(())
  }

  pub fn is_exist<K: AsRef<[u8]> + ?Sized>(&self, name: &K) -> bool {
    self.get_did(name).is_some()
  }

  pub fn load_doc<K: AsRef<[u8]> + ?Sized>(
    &self,
    name: &K,
    txn: &mut TransactionMut,
  ) -> Result<(), PersistenceError> {
    if let Some(did) = self.get_did(name) {
      let doc_state_key = make_doc_state_key(did);
      if let Some(doc_state) = self.db.get(doc_state_key)? {
        let update = Update::decode_v1(doc_state.as_ref())?;
        txn.apply_update(update);
      }

      let update_start = make_update_key(did, 0);
      let update_end = make_update_key(did, u32::MAX);
      let encoded_updates = self.db.batch_get(&update_start, &update_end)?;
      for encoded_update in encoded_updates {
        let update = Update::decode_v1(encoded_update.as_ref())?;
        txn.apply_update(update);
      }
      Ok(())
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }

  pub fn push_update<K: AsRef<[u8]> + ?Sized>(
    &self,
    name: &K,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let did = self.get_or_create_did(name.as_ref())?;
    let last_clock = {
      let end = make_update_key(did, u32::MAX);
      if let Some((k, _v)) = self.entry_before_key(&end) {
        let last_key = k.as_ref();
        let len = last_key.len();
        let last_clock = &last_key[(len - 5)..(len - 1)]; // update key scheme: 01{name:n}1{clock:4}0
        u32::from_be_bytes(last_clock.try_into().unwrap())
      } else {
        0
      }
    };
    let clock = last_clock + 1;
    let update_key = make_update_key(did, clock);
    self.db.insert(&update_key, update)?;
    Ok(())
  }

  pub fn delete_doc<K: AsRef<[u8]> + ?Sized>(&self, name: &K) -> Result<(), PersistenceError> {
    if let Some(did) = self.get_did(name) {
      let key = make_doc_id(name.as_ref());
      let _ = self.db.remove(key);

      let start = make_doc_start_key(did);
      let end = make_doc_end_key(did);
      let _ = self.db.batch_remove(start, end);

      let doc_state_key = make_doc_state_key(did);
      let sv_key = make_state_vector_key(did);
      let _ = self.db.remove(doc_state_key);
      let _ = self.db.remove(sv_key);
    }
    Ok(())
  }

  pub fn get_all_docs(&self) -> Result<DocsNameIter, PersistenceError> {
    let from = Key::from_const([SPACE, DID_SPACE]);
    let to = Key::from_const([SPACE, DOC_SPACE]);
    let iter = self.db.range(from..=to);

    Ok(DocsNameIter { iter })
  }

  pub fn get_updates<K: AsRef<[u8]> + ?Sized>(
    &self,
    name: &K,
  ) -> Result<Vec<Update>, PersistenceError> {
    if let Some(did) = self.get_did(name) {
      let start = make_update_key(did, 0);
      let end = make_update_key(did, u32::MAX);
      let encoded_updates = self.db.batch_get(&start, &end)?;
      let mut updates = vec![];
      for encoded_update in encoded_updates {
        updates.push(Update::decode_v1(encoded_update.as_ref())?);
      }
      Ok(updates)
    } else {
      Err(PersistenceError::DocumentNotExist)
    }
  }

  fn get_or_create_did<K: AsRef<[u8]> + ?Sized>(
    &self,
    name: &K,
  ) -> Result<DocID, PersistenceError> {
    if let Some(did) = self.get_did(name.as_ref()) {
      Ok(did)
    } else {
      let last_did = self
        .did_before_key([SPACE, DOC_SPACE].as_ref())
        .unwrap_or(0);
      let new_did = last_did + 1;
      let key = make_doc_id(name.as_ref());
      let _ = self.db.insert(key, &new_did.to_be_bytes());
      Ok(new_did)
    }
  }

  fn get_did<K: AsRef<[u8]> + ?Sized>(&self, name: &K) -> Option<DocID> {
    let key = make_doc_id(name.as_ref());
    let value = self.db.get(key).ok()??;
    Some(DocID::from_be_bytes(value.as_ref().try_into().unwrap()))
  }

  /// Looks into the last entry value prior to a given key.
  fn entry_before_key(&self, key: &[u8]) -> Option<(IVec, IVec)> {
    let (k, v) = self.db.get_lt(key).ok()??;
    Some((k, v))
  }

  fn did_before_key(&self, key: &[u8]) -> Option<DocID> {
    let (_, v) = self.entry_before_key(key)?;
    Some(DocID::from_be_bytes(v.as_ref().try_into().ok()?))
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
