use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use yrs::updates::encoder::{Encoder, EncoderV1};
use yrs::ReadTxn;

use crate::db::{batch_get, batch_remove};
use crate::keys::{make_snapshot_id_key, make_snapshot_update_key, SnapshotID};
use crate::{DbContext, PersistenceError};

pub struct YrsSnapshotDB<'a> {
  pub(crate) context: &'a DbContext,
  pub(crate) uid: i64,
}

impl<'a> YrsSnapshotDB<'a> {
  pub fn push_snapshot<K: AsRef<[u8]> + ?Sized, T: ReadTxn>(
    &self,
    object_id: &K,
    description: String,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let data = encode_snapshot(txn);
    let snapshot = CollabSnapshot::new(data, description).to_vec();
    let snapshot_id = self.get_or_create_snapshot_id(object_id.as_ref())?;
    self.context.insert_snapshot_update(snapshot_id, snapshot)?;
    Ok(())
  }

  pub fn get_snapshots<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Vec<CollabSnapshot> {
    let mut snapshots = vec![];
    if let Some(snapshot_id) = self.get_snapshot_id(object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, SnapshotID::MAX);
      if let Ok(encoded_snapshots) = batch_get(&self.context.db.read(), &start, &end) {
        for encoded_snapshot in encoded_snapshots {
          if let Ok(snapshot) = CollabSnapshot::try_from(encoded_snapshot.as_ref()) {
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
    if let Some(snapshot_id) = self.get_snapshot_id(object_id) {
      let start = make_snapshot_update_key(snapshot_id, 0);
      let end = make_snapshot_update_key(snapshot_id, SnapshotID::MAX);
      batch_remove(&mut self.context.db.write(), &start, &end)?;
    }
    Ok(())
  }

  fn get_or_create_snapshot_id<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<SnapshotID, PersistenceError> {
    if let Some(snapshot_id) = self.get_snapshot_id(object_id.as_ref()) {
      Ok(snapshot_id)
    } else {
      let key = make_snapshot_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
      let new_snapshot_id = self.context.create_snapshot_id_for_key(key)?;
      Ok(new_snapshot_id)
    }
  }

  fn get_snapshot_id<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Option<SnapshotID> {
    let key = make_snapshot_id_key(&self.uid.to_be_bytes(), object_id.as_ref());
    self.context.get_snapshot_id_for_key(key)
  }
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
