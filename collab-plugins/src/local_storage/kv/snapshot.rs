use std::fmt::Debug;
use std::panic;
use std::panic::AssertUnwindSafe;

use crate::local_storage::kv::keys::*;
use crate::local_storage::kv::*;
use collab_entity::CollabType;
use serde::{Deserialize, Serialize};
use yrs::updates::encoder::{Encoder, EncoderV1};
use yrs::{ReadTxn, Snapshot};

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
  fn create_snapshot<K, T>(
    &self,
    uid: i64,
    object_id: &K,
    txn: &T,
    snapshot: Snapshot,
  ) -> Result<(), PersistenceError>
  where
    K: AsRef<[u8]> + ?Sized + Debug,
    T: ReadTxn,
  {
    match try_encode_snapshot(txn, snapshot) {
      Ok(data) => {
        if data.is_empty() {
          tracing::warn!("🟡unexpected empty snapshot for object_id: {:?}", object_id);
          return Ok(());
        }
        tracing::trace!("New snapshot for object:{:?}", object_id);
        let snapshot_id = self.create_snapshot_id(uid, object_id.as_ref())?;
        insert_snapshot_update(self, snapshot_id, object_id, data)?;
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

  fn create_snapshot_with_data<K>(
    &self,
    uid: i64,
    object_id: &K,
    snapshot_data: Vec<u8>,
  ) -> Result<(), PersistenceError>
  where
    K: AsRef<[u8]> + ?Sized + Debug,
  {
    tracing::trace!("New snapshot for object:{:?}", object_id);
    let snapshot_id = self.create_snapshot_id(uid, object_id.as_ref())?;
    insert_snapshot_update(self, snapshot_id, object_id, snapshot_data)?;
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

  fn get_last_snapshot_by_snapshot_id(&self, snapshot_id: SnapshotID) -> Option<CollabSnapshot> {
    let last_update_key = self.get_snapshot_last_update_key(snapshot_id)?;
    self.get(last_update_key.as_ref()).ok()?.and_then(|value| {
      if let Ok(snapshot) = CollabSnapshot::try_from(value.as_ref()) {
        Some(snapshot)
      } else {
        None
      }
    })
  }

  fn get_last_snapshot<K: AsRef<[u8]> + ?Sized>(
    &self,
    uid: i64,
    object_id: &K,
  ) -> Option<CollabSnapshot> {
    let snapshot_id = get_snapshot_id(uid, self, object_id)?;
    self.get_last_snapshot_by_snapshot_id(snapshot_id)
  }

  fn delete_last_snapshot_by_snapshot_id(&self, snapshot_id: SnapshotID) {
    if let Some(last_update_key) = self.get_snapshot_last_update_key(snapshot_id) {
      match self.remove(last_update_key.as_ref()) {
        Ok(_) => {},
        Err(e) => {
          tracing::error!("🔴delete last snapshot failed: {:?}", e);
        },
      }
    }
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
      let new_snapshot_id = insert_doc_id_for_key(self, key)?;
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

pub fn try_encode_snapshot<T: ReadTxn>(
  txn: &T,
  snapshot: Snapshot,
) -> Result<Vec<u8>, PersistenceError> {
  let mut encoded_data = vec![];
  let result = {
    let mut wrapper = AssertUnwindSafe(&mut encoded_data);
    let wrapper_txn = AssertUnwindSafe(txn);
    panic::catch_unwind(move || {
      let mut encoder = EncoderV1::new();
      wrapper_txn
        .encode_state_from_snapshot(&snapshot, &mut encoder)
        .unwrap();
      **wrapper = encoder.to_vec();
    })
  };
  match result {
    Ok(_) => Ok(encoded_data),
    Err(e) => Err(PersistenceError::InvalidData(format!("{:?}", e))),
  }
}

pub trait SnapshotPersistence: Send + Sync {
  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    collab_type: &CollabType,
    encoded_v1: Vec<u8>,
  ) -> Result<(), PersistenceError>;
}

#[derive(Serialize, Deserialize)]
pub struct CollabSnapshot {
  pub data: Vec<u8>,
  pub created_at: i64,
}

impl CollabSnapshot {
  pub fn new(data: Vec<u8>) -> CollabSnapshot {
    let created_at = chrono::Utc::now().timestamp();
    Self { data, created_at }
  }

  pub fn to_vec(&self) -> Vec<u8> {
    bincode::serialize(&self).unwrap()
  }
}

impl TryFrom<&[u8]> for CollabSnapshot {
  type Error = PersistenceError;

  fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
    Ok(bincode::deserialize(value)?)
  }
}
