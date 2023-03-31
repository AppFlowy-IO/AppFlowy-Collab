use crate::keys::{
  clock_from_key, make_snapshot_id, make_snapshot_key, SnapshotID, SNAPSHOT_SPACE,
  SNAPSHOT_SPACE_OBJECT,
};
use crate::{CollabKV, PersistenceError};
use serde::{Deserialize, Serialize};
use sled::IVec;
use std::collections::HashMap;

use yrs::updates::encoder::{Encoder, EncoderV1};
use yrs::ReadTxn;

pub struct YrsSnapshot<'a> {
  pub(crate) db: &'a CollabKV,
  pub(crate) uid: i64,
}

impl<'a> YrsSnapshot<'a> {
  pub fn push_snapshot<K: AsRef<[u8]> + ?Sized, T: ReadTxn>(
    &self,
    object_id: &K,
    description: String,
    txn: &T,
  ) -> Result<(), PersistenceError> {
    let data = encode_snapshot(txn);
    let snapshot = CollabSnapshot::new(data, description).to_vec();
    let snapshot_id = self.get_or_create_snapshot_id(object_id.as_ref())?;
    let clock = self.get_next_clock(snapshot_id);
    let update_key = make_snapshot_key(snapshot_id, clock);
    self.db.insert(update_key, &snapshot)?;
    Ok(())
  }

  pub fn get_snapshots<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Vec<CollabSnapshot> {
    let mut snapshots = vec![];
    if let Some(snapshot_id) = self.get_snapshot_id(object_id) {
      let start = make_snapshot_key(snapshot_id, 0);
      let end = make_snapshot_key(snapshot_id, u32::MAX);
      if let Ok(encoded_snapshots) = self.db.batch_get(&start, &end) {
        for encoded_snapshot in encoded_snapshots {
          if let Ok(snapshot) = CollabSnapshot::try_from(encoded_snapshot.as_ref()) {
            snapshots.push(snapshot);
          }
        }
      }
    }
    snapshots
  }

  fn get_or_create_snapshot_id<K: AsRef<[u8]> + ?Sized>(
    &self,
    object_id: &K,
  ) -> Result<SnapshotID, PersistenceError> {
    if let Some(snapshot_id) = self.get_snapshot_id(object_id.as_ref()) {
      Ok(snapshot_id)
    } else {
      let last_snapshot_id = self
        .snapshot_id_before_key([SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT].as_ref())
        .unwrap_or(0);
      let new_snapshot_id = last_snapshot_id + 1;
      let key = make_snapshot_id(&self.uid.to_be_bytes(), object_id.as_ref());
      let _ = self.db.insert(key, &new_snapshot_id.to_be_bytes());
      Ok(new_snapshot_id)
    }
  }

  fn get_snapshot_id<K: AsRef<[u8]> + ?Sized>(&self, object_id: &K) -> Option<SnapshotID> {
    let key = make_snapshot_id(&self.uid.to_be_bytes(), object_id.as_ref());
    let value = self.db.get(key).ok()??;
    Some(SnapshotID::from_be_bytes(
      value.as_ref().try_into().unwrap(),
    ))
  }

  fn snapshot_id_before_key(&self, key: &[u8]) -> Option<SnapshotID> {
    let (_, v) = self.entry_before_key(key)?;
    Some(SnapshotID::from_be_bytes(v.as_ref().try_into().ok()?))
  }

  fn entry_before_key(&self, key: &[u8]) -> Option<(IVec, IVec)> {
    let (k, v) = self.db.get_lt(key).ok()??;
    Some((k, v))
  }

  fn get_next_clock(&self, snapshot_id: SnapshotID) -> u32 {
    let last_clock = {
      let end = make_snapshot_key(snapshot_id, u32::MAX);
      if let Some((k, _v)) = self.entry_before_key(&end) {
        let last_key = k.as_ref();
        let last_clock = clock_from_key(last_key); // update key scheme: 01{name:n}1{clock:4}0
        u32::from_be_bytes(last_clock.try_into().unwrap())
      } else {
        0
      }
    };
    last_clock + 1
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
