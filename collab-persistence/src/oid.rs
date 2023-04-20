use std::time::SystemTime;

use parking_lot::Mutex;

use lazy_static::lazy_static;

const EPOCH: u64 = 1637806706000;
const NODE_BITS: u64 = 8;
const SEQUENCE_BITS: u64 = 12;
const TIMESTAMP_BITS: u64 = 42;
const NODE_ID_SHIFT: u64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u64 = NODE_BITS + SEQUENCE_BITS;
const SCOPE_SHIFT: u64 = TIMESTAMP_BITS + TIMESTAMP_SHIFT;
const SEQUENCE_MASK: u64 = (1 << SEQUENCE_BITS) - 1;

pub type OID = u64;
pub const OID_LEN: usize = 8;

lazy_static! {
  pub static ref OID_GEN: Mutex<OIDGen> = Mutex::new(OIDGen::new(1));
}

pub struct OIDGen {
  node_id: u64,
  sequence: u64,
  last_timestamp: u64,
}

impl OIDGen {
  pub fn new(node_id: u64) -> OIDGen {
    OIDGen {
      node_id,
      sequence: 0,
      last_timestamp: 0,
    }
  }

  pub fn next_id(&mut self) -> u64 {
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
    2 << SCOPE_SHIFT
      | (timestamp - EPOCH) << TIMESTAMP_SHIFT
      | self.node_id << NODE_ID_SHIFT
      | self.sequence
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
//
//   use crate::oid::OID_GEN;
//
//   #[test]
//   fn test_oid_gen() {
//     let mut map = Arc::new(RwLock::new(HashMap::new()));
//
//     let mut handles = vec![];
//     for i in 0..1000 {
//       let cloned_map = map.clone();
//       let handle = thread::spawn(move || {
//         let id = OID_GEN.lock().next_id();
//         println!("id: {}", id);
//         println!("id: {:b}", id);
//         println!("id: {:?}", id.to_be_bytes().as_ref());
//         if cloned_map.read().contains_key(&id) {
//           panic!("id: {} is duplicated!", id);
//         }
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
