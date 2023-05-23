use std::fmt::Debug;
use std::panic;
use std::panic::AssertUnwindSafe;

use serde::{Deserialize, Serialize};
use yrs::updates::encoder::{Encoder, EncoderV1};
use yrs::ReadTxn;

use crate::keys::{make_snapshot_id_key, make_snapshot_update_key, Clock, Key, SnapshotID};
use crate::kv::KVEntry;
use crate::kv::KVStore;
use crate::{
  get_id_for_key, get_last_update_key, insert_snapshot_update, make_doc_id_for_key,
  PersistenceError,
};

impl<'a, T> SnapshotAction<'a> for T
where
  T: KVStore<'a>,
  PersistenceError: From<<Self as KVStore<'a>>::Error>,
{
}

pub trait SnapshotAction<'a>: KVStore<'a> + Sized
where
  PersistenceError: From<<Self as KVStore<'a>>::Error>,
{
  /// Create a snapshot for the given object id.
  /// The snapshot contains the updates prior to the given update_key. For example,
  /// if the update_key is 10, the snapshot will contain updates 0-9. So when restoring
  /// the document from a snapshot, it should apply the update from key:10.
  fn push_snapshot<K1, K2, T>(
    &self,
    uid: i64,
    object_id: &K1,
    update_key: K2,
    txn: &T,
  ) -> Result<(), PersistenceError>
  where
    K1: AsRef<[u8]> + ?Sized + Debug,
    K2: Into<Vec<u8>>,
    T: ReadTxn,
  {
    match try_encode_snapshot(txn) {
      Ok(data) => {
        if data.is_empty() {
          tracing::warn!("🟡unexpected empty snapshot for object_id: {:?}", object_id);
          return Ok(());
        }
        tracing::trace!("New snapshot for object:{:?}", object_id);
        let snapshot_id = self.create_snapshot_id(uid, object_id.as_ref())?;
        insert_snapshot_update(self, update_key, snapshot_id, object_id, data)?;
      },
      Err(e) => {
        tracing::error!(
          "🔴failed to encode snapshot for object_id: {:?}, error: {:?}",
          object_id,
          e
        );
      },
    }
    Ok(())
  }

  /// Return list of snapshots for the given object id.
  fn get_snapshots<K: AsRef<[u8]> + ?Sized>(&self, uid: i64, object_id: &K) -> Vec<CollabSnapshot> {
    let mut snapshots = vec![];
    if let Some(snapshot_id) = get_snapshot_id(uid, self, object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, Clock::MAX);

      if let Ok(encoded_updates) = self.range(start.as_ref()..=end.as_ref()) {
        for encoded_snapshot in encoded_updates {
          if let Ok(snapshot) = CollabSnapshot::try_from(encoded_snapshot.value()) {
            snapshots.push(snapshot);
          }
        }
      }
    }
    snapshots
  }

  fn get_last_snapshot_update(&self, snapshot_id: SnapshotID) -> Option<CollabSnapshot> {
    let last_update_key = self.get_snapshot_last_update_key(snapshot_id)?;
    self.get(last_update_key.as_ref()).ok()?.and_then(|value| {
      if let Ok(snapshot) = CollabSnapshot::try_from(value.as_ref()) {
        Some(snapshot)
      } else {
        None
      }
    })
  }

  /// Delete all snapshots for the given object id.
  fn delete_all_snapshots<K: AsRef<[u8]> + ?Sized>(
    &self,
    uid: i64,
    object_id: &K,
  ) -> Result<(), PersistenceError> {
    if let Some(snapshot_id) = get_snapshot_id(uid, self, object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, Clock::MAX);
      self.remove_range(start.as_ref(), end.as_ref())?;
    }
    Ok(())
  }

  fn delete_last_snapshot_update(&self, snapshot_id: SnapshotID) {
    if let Some(last_update_key) = self.get_snapshot_last_update_key(snapshot_id) {
      match self.remove(last_update_key.as_ref()) {
        Ok(_) => {},
        Err(e) => {
          tracing::error!("Failed to delete last snapshot update: {:?}", e);
        },
      }
    }
  }

  /// Create a snapshot id for the given object id.
  fn create_snapshot_id<K: AsRef<[u8]> + ?Sized>(
    &self,
    uid: i64,
    object_id: &K,
  ) -> Result<SnapshotID, PersistenceError> {
    if let Some(snapshot_id) = get_snapshot_id(uid, self, object_id.as_ref()) {
      Ok(snapshot_id)
    } else {
      let key = make_snapshot_id_key(&uid.to_be_bytes(), object_id.as_ref());
      let new_snapshot_id = make_doc_id_for_key(self, key)?;
      Ok(new_snapshot_id)
    }
  }

  fn get_snapshot_last_update_key(&self, snapshot_id: SnapshotID) -> Option<Key<16>> {
    get_last_update_key(self, snapshot_id, make_snapshot_update_key).ok()
  }
}

pub fn get_snapshot_id<'a, K, S>(uid: i64, store: &S, object_id: &K) -> Option<SnapshotID>
where
  K: AsRef<[u8]> + ?Sized,
  S: KVStore<'a>,
{
  let key = make_snapshot_id_key(&uid.to_be_bytes(), object_id.as_ref());
  get_id_for_key(store, key)
}

fn try_encode_snapshot<T: ReadTxn>(txn: &T) -> Result<Vec<u8>, PersistenceError> {
  let snapshot = txn.snapshot();
  let mut encoded_data = vec![];
  match {
    let mut wrapper = AssertUnwindSafe(&mut encoded_data);
    let wrapper_txn = AssertUnwindSafe(txn);
    panic::catch_unwind(move || {
      let mut encoder = EncoderV1::new();
      wrapper_txn
        .encode_state_from_snapshot(&snapshot, &mut encoder)
        .unwrap();
      **wrapper = encoder.to_vec();
    })
  } {
    Ok(_) => Ok(encoded_data),
    Err(_) => Err(PersistenceError::InternalError),
  }
}

#[derive(Serialize, Deserialize)]
pub struct CollabSnapshot {
  pub data: Vec<u8>,
  pub created_at: i64,
  pub update_key: Vec<u8>,
}

impl CollabSnapshot {
  pub fn new(data: Vec<u8>, update_key: Vec<u8>) -> CollabSnapshot {
    let created_at = chrono::Utc::now().timestamp();
    Self {
      data,
      created_at,
      update_key,
    }
  }

  pub fn to_vec(&self) -> Vec<u8> {
    bincode::serialize(&self).unwrap()
  }
}

impl TryFrom<&[u8]> for CollabSnapshot {
  type Error = PersistenceError;

  fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
    let value = bincode::deserialize(value)?;
    Ok(value)
  }
}
