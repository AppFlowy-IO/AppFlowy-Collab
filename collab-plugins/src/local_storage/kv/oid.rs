#![allow(clippy::upper_case_acronyms)]

use lazy_static::lazy_static;
use std::cmp;
use std::sync::{Arc, atomic};

use crate::{if_native, if_wasm};

//    scope        system time since epoch         node id sequence number
//   |<-2->| <-----------------41--------------->| <--8--> |<----12---->|
//           101010100000011010101111101100101000
// 0      10 101010100000011010101111101100101000 00000000 000000000000
pub type OID = u64;

#[derive(Copy, Clone, Debug)]
pub struct DocIDGen;

lazy_static! {
  static ref LAST_OID: Arc<atomic::AtomicU64> = Arc::new(atomic::AtomicU64::new(0));
  static ref NODE_ID: Arc<atomic::AtomicU64> = Arc::new(atomic::AtomicU64::new(0));
}

impl DocIDGen {
  const EPOCH: u64 = 1637806706000;
  const NODE_BITS: u64 = 8;
  const SEQUENCE_BITS: u64 = 12;
  const TIMESTAMP_BITS: u64 = 41;
  const TIMESTAMP_MASK: u64 = ((1 << Self::TIMESTAMP_BITS) - 1) << Self::TIMESTAMP_SHIFT;
  const NODE_ID_SHIFT: u64 = Self::SEQUENCE_BITS;
  const TIMESTAMP_SHIFT: u64 = Self::NODE_BITS + Self::SEQUENCE_BITS;
  const SCOPE_SHIFT: u64 = Self::TIMESTAMP_BITS + Self::TIMESTAMP_SHIFT;
  const SEQUENCE_MASK: u64 = (1 << Self::SEQUENCE_BITS) - 1;

  #[inline]
  fn node_id() -> u64 {
    NODE_ID.load(atomic::Ordering::Relaxed)
  }

  pub fn set_node_id(node_id: u8) {
    let shifted = (node_id as u64) << Self::NODE_ID_SHIFT;
    NODE_ID.store(shifted, atomic::Ordering::Relaxed)
  }

  #[inline]
  fn create_oid(timestamp: u64, sequence: u64) -> OID {
    (2 << Self::SCOPE_SHIFT) | (timestamp << Self::TIMESTAMP_SHIFT) | Self::node_id() | sequence
  }

  #[inline]
  fn get_timestamp(oid: OID) -> u64 {
    (oid & Self::TIMESTAMP_MASK) >> Self::TIMESTAMP_SHIFT
  }

  pub fn next_id() -> OID {
    let mut last_oid = LAST_OID.load(atomic::Ordering::Relaxed);
    loop {
      let current_timestamp = Self::timestamp();
      let last_timestamp = Self::get_timestamp(last_oid);
      let sequence = match last_timestamp.cmp(&current_timestamp) {
        cmp::Ordering::Less => {
          // The timestamp increased, so reset the sequence number
          0
        },
        cmp::Ordering::Equal => {
          // The timestamp didn't change, so increment the sequence number
          let sequence = (last_oid + 1) & Self::SEQUENCE_MASK;
          if sequence == 0 {
            // The sequence number wrapped around, so wait until the next millisecond
            continue;
          } else {
            sequence
          }
        },
        cmp::Ordering::Greater => {
          panic!("Clock moved backwards!");
        },
      };
      let new_oid = Self::create_oid(current_timestamp, sequence);
      match LAST_OID.compare_exchange_weak(
        last_oid,
        new_oid,
        atomic::Ordering::Relaxed,
        atomic::Ordering::Relaxed,
      ) {
        Ok(_) => {
          return new_oid;
        },
        Err(current_oid) => {
          // failed to generate a new OID due to concurrent operation, try again
          last_oid = current_oid;
        },
      }
    }
  }

  if_wasm! {
     fn timestamp() -> u64 {
      js_sys::Date::now() as u64 - Self::EPOCH
     }
  }

  if_native! {
    fn timestamp() -> u64 {
      std::time::SystemTime::now()
      .duration_since(std::time::SystemTime::UNIX_EPOCH)
      .expect("Clock moved backwards!")
      .as_millis() as u64 - Self::EPOCH
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::local_storage::kv::oid::DocIDGen;
  use std::collections::HashMap;
  use std::sync::{Arc, RwLock};
  use std::thread;

  #[test]
  fn test_oid_gen() {
    let map = Arc::new(RwLock::new(HashMap::new()));

    let mut handles = vec![];
    for _i in 0..2 {
      let cloned_map = map.clone();
      let handle = thread::spawn(move || {
        let id = DocIDGen::next_id();
        // println!("id: {:b}", a.last_timestamp - EPOCH);
        // println!("id: {:b}", id);
        if cloned_map.read().unwrap().contains_key(&id) {
          panic!("id: {} is duplicated!", id);
        }
        cloned_map.write().unwrap().insert(id, id);
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }
}
