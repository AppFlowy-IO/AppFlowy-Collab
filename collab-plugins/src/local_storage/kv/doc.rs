use crate::local_storage::kv::keys::*;
use crate::local_storage::kv::snapshot::SnapshotAction;
use crate::local_storage::kv::*;
use smallvec::{SmallVec, smallvec};
use std::collections::HashSet;
use tracing::{error, info};
use uuid::Uuid;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, TransactionMut, Update};

pub trait CollabKVAction<'a>: KVStore<'a> + Sized + 'a
where
  PersistenceError: From<<Self as KVStore<'a>>::Error>,
{
  /// Create a new document with the given object id.
  fn create_new_doc<T: ReadTxn>(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    if self.is_exist(uid, workspace_id, object_id) {
      tracing::warn!("🟡{:?} already exist", object_id);
      return Err(PersistenceError::DocumentAlreadyExist);
    }
    let doc_id = get_or_create_did(uid, self, workspace_id, object_id)?;
    let doc_state = txn.encode_diff_v1(&StateVector::default());
    let sv = txn.state_vector().encode_v1();
    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);

    info!("new doc:{:?}, doc state len:{}", object_id, doc_state.len());
    self.insert(doc_state_key, doc_state)?;
    self.insert(sv_key, sv)?;

    Ok(())
  }

  fn upsert_doc_with_doc_state(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    state_vector: Vec<u8>,
    doc_state: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    let doc_id = get_or_create_did(uid, self, workspace_id, object_id)?;
    let start = make_doc_start_key(doc_id);
    let end = make_doc_end_key(doc_id);
    self.remove_range(start.as_ref(), end.as_ref())?;

    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);

    info!("new doc:{:?}, doc state len:{}", object_id, doc_state.len());
    self.insert(doc_state_key, doc_state)?;
    self.insert(sv_key, state_vector)?;

    Ok(())
  }

  /// Flushes the document state and state vector to the storage.
  ///
  /// This function writes the state of a document, identified by a unique `object_id`, along with its
  /// associated state vector to the persistent storage. It first ensures that a document ID is
  /// assigned or retrieved for the given user ID and object identifier. Then, it proceeds to clear any
  /// existing state for that document from the storage before inserting the new state and state vector.
  ///
  fn flush_doc(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    state_vector: Vec<u8>,
    doc_state: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    let doc_id = get_or_create_did(uid, self, workspace_id, object_id)?;

    // Remove the updates
    let start = make_doc_start_key(doc_id);
    let end = make_doc_end_key(doc_id);
    self.remove_range(start.as_ref(), end.as_ref())?;

    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);
    // Insert new doc state and state vector
    self.insert(doc_state_key, doc_state)?;
    self.insert(sv_key, state_vector)?;
    Ok(())
  }

  fn is_exist(&self, uid: i64, workspace_id: &str, object_id: &str) -> bool {
    get_doc_id(uid, self, workspace_id, object_id).is_some()
  }

  /// Load the document from the database and apply the updates to the transaction.
  /// It will try to load the document in these two ways:
  ///   1. D = document state + updates
  ///   2. D = document state + snapshot + updates
  ///
  /// Return the number of updates
  fn load_doc_with_txn(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    txn: &mut TransactionMut,
  ) -> Result<u32, PersistenceError> {
    let mut update_count = 0;

    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let doc_state_key = make_doc_state_key(doc_id);
      if let Some(doc_state) = self.get(doc_state_key.as_ref())? {
        // Load the doc state

        match Update::decode_v1(doc_state.as_ref()) {
          Ok(update) => {
            txn.try_apply_update(update)?;
          },
          Err(err) => {
            error!("🔴{:?} decode doc state error: {}", object_id, err);
            return Err(PersistenceError::Yrs(err));
          },
        }

        // If the enable_snapshot is true, we will try to load the snapshot.
        let update_start = make_doc_update_key(doc_id, 0).to_vec();
        let update_end = make_doc_update_key(doc_id, Clock::MAX);

        // Load the updates
        let encoded_updates = self.range(update_start.as_ref()..update_end.as_ref())?;
        for encoded_update in encoded_updates {
          // Decode the update and apply it to the transaction. If the update is invalid, we will
          // remove the update and the following updates.
          if let Err(e) = Update::decode_v1(encoded_update.value())
            .map_err(PersistenceError::Yrs)
            .and_then(|update| {
              // trace!("apply update: {:#?}", update);
              txn.try_apply_update(update)
            })
          {
            tracing::error!("🔴{:?} apply update error: {}", object_id, e);
            self.remove_range(encoded_update.key().as_ref(), update_end.as_ref())?;
            break;
          }
          update_count += 1;
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
      tracing::trace!("[Client] => {:?} not exist", object_id);
      Err(PersistenceError::RecordNotFound(format!(
        "doc with given object id: {:?} is not found",
        object_id
      )))
    }
  }

  fn load_doc(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    doc: &Doc,
  ) -> Result<u32, PersistenceError> {
    let mut txn = doc.transact_mut();
    self.load_doc_with_txn(uid, workspace_id, object_id, &mut txn)
  }

  /// Push an update to the persistence
  fn push_update(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    update: &[u8],
  ) -> Result<Vec<u8>, PersistenceError> {
    match get_doc_id(uid, self, workspace_id, object_id) {
      None => {
        tracing::error!(
          "🔴Insert update failed. Can't find the doc for {}-{:?}",
          uid,
          object_id
        );
        Err(PersistenceError::RecordNotFound(format!(
          "doc with given object id: {:?} is not found",
          object_id
        )))
      },
      Some(doc_id) => insert_doc_update(self, doc_id, object_id, update.to_vec()),
    }
  }

  /// Delete the updates that prior to the given key. The given key is not included.
  fn delete_updates_to(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    end: &[u8],
  ) -> Result<(), PersistenceError> {
    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      self.remove_range(start.as_ref(), end.as_ref())?;
    }
    Ok(())
  }

  fn delete_all_updates(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Result<(), PersistenceError> {
    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);
      self.remove_range(start.as_ref(), end.as_ref())?;
    }
    Ok(())
  }

  fn flush_doc_with(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
    doc_state: &[u8],
    sv: &[u8],
  ) -> Result<(), PersistenceError> {
    let doc_id = get_or_create_did(uid, self, workspace_id, object_id)?;
    let start = make_doc_start_key(doc_id);
    let end = make_doc_end_key(doc_id);
    self.remove_range(start.as_ref(), end.as_ref())?;

    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);

    // Insert new doc state and state vector
    self.insert(doc_state_key, doc_state)?;
    self.insert(sv_key, sv)?;
    Ok(())
  }

  fn get_all_updates(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Result<Vec<Vec<u8>>, PersistenceError> {
    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);
      let range = self.range(start.as_ref()..end.as_ref())?;
      let mut updates = vec![];
      for update in range {
        updates.push(update.value().to_vec());
      }
      Ok(updates)
    } else {
      Ok(vec![])
    }
  }

  /// Delete the document from the persistence
  /// This will remove all the updates and the document state
  fn delete_doc(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Result<(), PersistenceError> {
    if let Some(did) = get_doc_id(uid, self, workspace_id, object_id) {
      tracing::trace!("[Client {}] => [{}] delete {:?} doc", uid, did, object_id);
      let key = make_doc_id_key_v1(
        &uid.to_be_bytes(),
        workspace_id.as_ref(),
        object_id.as_ref(),
      );
      let _ = self.remove(key.as_ref());

      // Delete the updates
      let start = make_doc_start_key(did);
      let end = make_doc_end_key(did);
      self.remove_range(start.as_ref(), end.as_ref())?;

      // Delete the document state and the state vector
      let doc_state_key = make_doc_state_key(did);
      let sv_key = make_state_vector_key(did);
      let _ = self.remove(doc_state_key.as_ref());
      let _ = self.remove(sv_key.as_ref());

      // Delete the snapshot
      self.delete_all_snapshots(uid, object_id)?;
    }
    Ok(())
  }

  fn get_all_docs(
    &self,
  ) -> Result<OIDIter<<Self as KVStore<'a>>::Range, <Self as KVStore<'a>>::Entry>, PersistenceError>
  {
    let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
    let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);
    let iter = self.range(from.as_ref()..to.as_ref())?;
    Ok(OIDIter { iter })
  }

  fn get_all_object_ids(
    &self,
    uid: i64,
    workspace_id: &str,
  ) -> Result<impl Iterator<Item = String>, PersistenceError> {
    let uid_bytes = uid.to_be_bytes();
    let workspace_bytes = workspace_id.as_bytes();

    // Construct the `from` key with UID and workspace_id
    let mut from_vec: SmallVec<[u8; 24]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT];
    from_vec.extend_from_slice(&uid_bytes);
    from_vec.extend_from_slice(workspace_bytes);
    let from = Key(from_vec);

    // Construct the `to` key by appending 0xFF to cover the full range of keys with the same prefix
    let to_vec: SmallVec<[u8; 24]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
    let to = Key(to_vec);

    let iter = self.range(from.as_ref()..to.as_ref())?;

    Ok(iter.filter_map(move |entry| {
      extract_object_id_from_key_v1(entry.key(), uid_bytes.len(), workspace_bytes.len())
        .and_then(|object_id_bytes| String::from_utf8(object_id_bytes.to_vec()).ok())
    }))
  }

  fn get_all_workspace_ids(&self) -> Result<Vec<String>, PersistenceError> {
    let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
    let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);
    let iter = self.range(from.as_ref()..to.as_ref())?;

    let mut workspace_ids = HashSet::new();
    // Iterate over the keys and extract workspace IDs
    for entry in iter {
      let key_bytes = entry.key();
      if let Some(workspace_id) = extract_uuid_from_key(key_bytes) {
        workspace_ids.insert(Uuid::from_bytes(workspace_id).to_string());
      }
    }

    Ok(workspace_ids.into_iter().collect())
  }

  /// Return all the updates for the given document
  fn get_decoded_v1_updates(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Result<Vec<Update>, PersistenceError> {
    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);

      let mut updates = vec![];
      if let Ok(encoded_updates) = self.range(start.as_ref()..=end.as_ref()) {
        for encoded_update in encoded_updates {
          updates.push(Update::decode_v1(encoded_update.value())?);
        }
      }
      Ok(updates)
    } else {
      Err(PersistenceError::RecordNotFound(format!(
        "The document with given object id: {:?} is not found",
        object_id,
      )))
    }
  }

  fn get_doc_last_update_key(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Option<Key<16>> {
    let doc_id = get_doc_id(uid, self, workspace_id, object_id)?;
    get_last_update_key(self, doc_id, make_doc_update_key).ok()
  }

  /// Return the number of updates for the given document
  fn number_of_updates(&self, uid: i64, workspace_id: &str, object_id: &str) -> usize {
    if let Some(doc_id) = get_doc_id(uid, self, workspace_id, object_id) {
      let start = make_doc_update_key(doc_id, 0);
      let end = make_doc_update_key(doc_id, Clock::MAX);
      self
        .range(start.as_ref()..=end.as_ref())
        .map(|r| r.count())
        .unwrap_or(0)
    } else {
      0
    }
  }
}

impl<'a, T> CollabKVAction<'a> for T
where
  T: KVStore<'a> + 'a,
  PersistenceError: From<<Self as KVStore<'a>>::Error>,
{
}

/// Get or create a document id for the given object id.
fn get_or_create_did<'a, S>(
  uid: i64,
  store: &S,
  workspace_id: &str,
  object_id: &str,
) -> Result<DocID, PersistenceError>
where
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  if let Some(did) = get_doc_id(uid, store, workspace_id, object_id) {
    Ok(did)
  } else {
    let key = make_doc_id_key_v1(
      &uid.to_be_bytes(),
      workspace_id.as_ref(),
      object_id.as_ref(),
    );
    let new_did = insert_doc_id_for_key(store, key)?;
    Ok(new_did)
  }
}

fn get_doc_id<'a, S>(uid: i64, store: &S, workspace_id: &str, object_id: &str) -> Option<DocID>
where
  S: KVStore<'a>,
{
  let uid_bytes = uid.to_be_bytes();

  // Try to find the new key format first
  let new_key = make_doc_id_key_v1(&uid_bytes, workspace_id.as_ref(), object_id.as_ref());
  if let Some(doc_id) = get_id_for_key(store, new_key) {
    return Some(doc_id);
  }

  // Fallback to the old key format if not found
  let old_key = make_doc_id_key_v0(&uid_bytes, object_id.as_ref());
  get_id_for_key(store, old_key)
}

pub struct OIDIter<I, E>
where
  I: Iterator<Item = E>,
  E: KVEntry,
{
  iter: I,
}

impl<I, E> Iterator for OIDIter<I, E>
where
  I: Iterator<Item = E>,
  E: KVEntry,
{
  type Item = String;

  fn next(&mut self) -> Option<Self::Item> {
    let entry = self.iter.next()?;
    let content = oid_from_key(entry.key());
    Some(String::from_utf8_lossy(content).to_string())
  }
}
fn extract_uuid_from_key(key: &[u8]) -> Option<[u8; 16]> {
  // Start index is set to skip DOC_SPACE and DOC_SPACE_OBJECT (2 bytes)
  let start_index = 2;
  let end_index = start_index + 16;
  // Ensure the key has enough length for extracting a 16-byte UUID
  if key.len() >= end_index {
    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&key[start_index..end_index]);
    Some(uuid_bytes)
  } else {
    None
  }
}

pub fn migrate_old_keys<'a, S>(store: &'a S, workspace_id: &str) -> Result<(), PersistenceError>
where
  S: KVStore<'a>,
  PersistenceError: From<<S as KVStore<'a>>::Error>,
{
  let from = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT]);
  let to = Key::from_const([DOC_SPACE, DOC_SPACE_OBJECT_KEY]);

  let iter = store.range(from.as_ref()..to.as_ref())?;
  for entry in iter {
    let old_key = entry.key();
    let value = entry.value();
    let uid = &old_key[2..10];
    let object_id = &old_key[10..old_key.len() - 1];

    let new_key = make_doc_id_key_v1(uid, workspace_id.as_ref(), object_id);
    store.insert(new_key, value)?;
  }

  Ok(())
}

pub fn extract_object_id_from_key_v1(
  key: &[u8],
  uid_len: usize,
  workspace_id_len: usize,
) -> Option<&[u8]> {
  let prefix_len = 2; // DOC_SPACE + DOC_SPACE_OBJECT
  let start_index = prefix_len + uid_len + workspace_id_len;
  // Check if the key is long enough to contain an object_id and a terminator
  if key.len() > start_index && key[key.len() - 1] == TERMINATOR {
    Some(&key[start_index..key.len() - 1])
  } else {
    None
  }
}
