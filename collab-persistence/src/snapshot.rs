use std::collections::HashMap;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use yrs::updates::encoder::{Encoder, EncoderV1};
use yrs::ReadTxn;

use crate::keys::{make_snapshot_id_key, make_snapshot_update_key, Clock, SnapshotID};
use crate::kv::KVEntry;
use crate::kv::KV;
use crate::{
  create_id_for_key, get_id_for_key, insert_snapshot_update, PersistenceError, SubStore,
};

pub struct YrsSnapshotDB<'a, S> {
  pub(crate) store: &'a SubStore<S>,
  pub(crate) uid: i64,
}

impl<'a, S> YrsSnapshotDB<'a, S>
where
  S: KV,
  PersistenceError: From<<S as KV>::Error>,
{
  pub fn push_snapshot<K: AsRef<[u8]> + ?Sized + Debug, T: ReadTxn>(
    &self,
    object_id: &K,
    description: String,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let data = encode_snapshot(txn);
    let snapshot = CollabSnapshot::new(data, description).to_vec();
    let snapshot_id = self.create_snapshot_id(object_id.as_ref())?;
    insert_snapshot_update(&*self.store.read(), snapshot_id, object_id, snapshot)?;
    Ok(())
  }

  pub fn get_snapshots<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Vec<CollabSnapshot> {
    let mut snapshots = vec![];
    if let Some(snapshot_id) = get_snapshot_id(self.uid, &*self.store.read(), object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, Clock::MAX);

      if let Ok(encoded_updates) = self.store.read().iter_range(start.as_ref(), end.as_ref()) {
        for encoded_snapshot in encoded_updates {
          if let Ok(snapshot) = CollabSnapshot::try_from(encoded_snapshot.value()) {
            snapshots.push(snapshot);
          }
        }
      }
    }
    snapshots
  }

  pub fn delete_snapshot<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<(), PersistenceError> {
    let store = self.store.write();
    if let Some(snapshot_id) = get_snapshot_id(self.uid, &*store, object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, Clock::MAX);
      store.remove_range(start.as_ref(), end.as_ref())?;
    }
    Ok(())
  }

  fn create_snapshot_id<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<SnapshotID, PersistenceError> {
    let store = self.store.write();
    if let Some(snapshot_id) = get_snapshot_id(self.uid, &*store, object_id.as_ref()) {
      Ok(snapshot_id)
    } else {
      let key = make_snapshot_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let new_snapshot_id = create_id_for_key(&*store, key)?;
      Ok(new_snapshot_id)
    }
  }
}

fn get_snapshot_id<K, S>(uid: i64, store: &S, object_id: &K) -> Option<SnapshotID>
where
  K: AsRef<[u8]> + ?Sized,
  S: KV,
{
  let key = make_snapshot_id_key(&uid.to_be_bytes(), object_id.as_ref());
  get_id_for_key(store, key)
}

fn encode_snapshot<T: ReadTxn>(txn: &T) -> Vec<u8> {
  let snapshot = txn.snapshot();
  let mut encoder = EncoderV1::new();
  txn
    .encode_state_from_snapshot(&snapshot, &mut encoder)
    .unwrap();
  encoder.to_vec()
}

#[derive(Serialize, Deserialize)]
pub struct CollabSnapshot {
  pub data: Vec<u8>,
  pub created_at: i64,
  pub description: String,
  pub meta: HashMap<String, String>,
}

impl CollabSnapshot {
  pub fn new(data: Vec<u8>, description: String) -> CollabSnapshot {
    let created_at = chrono::Utc::now().timestamp();
    Self {
      data,
      created_at,
      description,
      meta: Default::default(),
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
