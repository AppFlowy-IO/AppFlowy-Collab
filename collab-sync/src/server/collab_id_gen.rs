#![allow(clippy::upper_case_acronyms)]

use std::time::SystemTime;

const EPOCH: u64 = 1637806706000;
const NODE_BITS: u64 = 8;
const SEQUENCE_BITS: u64 = 12;
const TIMESTAMP_BITS: u64 = 42;
const NODE_ID_SHIFT: u64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u64 = NODE_BITS + SEQUENCE_BITS;
const SCOPE_SHIFT: u64 = TIMESTAMP_BITS + TIMESTAMP_SHIFT;
const SEQUENCE_MASK: u64 = (1 << SEQUENCE_BITS) - 1;

pub type CollabId = i64;

pub const COLLAB_ID_LEN: usize = 8;

#[allow(dead_code)]
pub struct NonZeroNodeId(pub u64);

impl NonZeroNodeId {
  fn into_inner(self) -> u64 {
    if self.0 == 0 {
      panic!("Node ID cannot be zero!");
    }
    self.0
  }
}

pub struct CollabIDGen {
  node_id: u64,
  sequence: u64,
  last_timestamp: u64,
}

impl CollabIDGen {
  #[allow(dead_code)]
  pub fn new(node_id: NonZeroNodeId) -> CollabIDGen {
    CollabIDGen {
      node_id: node_id.into_inner(),
      sequence: 0,
      last_timestamp: 0,
    }
  }

  pub fn next_id(&mut self) -> CollabId {
    let timestamp = self.timestamp();
    if timestamp < self.last_timestamp {
      panic!("Clock moved backwards!");
    }

    if timestamp == self.last_timestamp {
      self.sequence = (self.sequence + 1) & SEQUENCE_MASK;
      if self.sequence == 0 {
        self.wait_next_millis();
      }
    } else {
      self.sequence = 0;
    }

    self.last_timestamp = timestamp;
    let id = 2 << SCOPE_SHIFT
      | (timestamp - EPOCH) << TIMESTAMP_SHIFT
      | self.node_id << NODE_ID_SHIFT
      | self.sequence;

    id as CollabId
  }

  fn wait_next_millis(&self) {
    let mut timestamp = self.timestamp();
    while timestamp == self.last_timestamp {
      timestamp = self.timestamp();
    }
  }

  fn timestamp(&self) -> u64 {
    SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .expect("Clock moved backwards!")
      .as_millis() as u64
  }
}

// #[cfg(test)]
// mod tests {
//   use std::collections::HashMap;
//   use std::sync::Arc;
//   use std::thread;
//
//   use parking_lot::RwLock;
//   use crate::oid::{EPOCH, OID_GEN};
//
//   #[test]
//   fn test_oid_gen() {
//     let mut map = Arc::new(RwLock::new(HashMap::new()));
//
//     let mut handles = vec![];
//     for i in 0..1 {
//       let cloned_map = map.clone();
//       let handle = thread::spawn(move || {
//         let mut a = OID_GEN.lock();
//         let id = a.next_id();
//         println!("id: {:b}", a.last_timestamp - EPOCH);
//         println!("id: {:b}", id);
//         if cloned_map.read().contains_key(&id) {
//           panic!("id: {} is duplicated!", id);
//         }
//         //          101001010010110010010010100011100111
//         // 10 000000101001010010110010010010100011100111 00000001 000000000000
//         cloned_map.write().insert(id, id);
//       });
//       handles.push(handle);
//     }
//
//     for handle in handles {
//       handle.join().unwrap();
//     }
//   }
// }
