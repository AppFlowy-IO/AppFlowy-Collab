#![allow(clippy::upper_case_acronyms)]

use crate::{if_native, if_wasm};
use lazy_static::lazy_static;
use parking_lot::Mutex;

const EPOCH: u64 = 1637806706000;
const NODE_BITS: u64 = 8;
const SEQUENCE_BITS: u64 = 12;
const TIMESTAMP_BITS: u64 = 41;
const NODE_ID_SHIFT: u64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u64 = NODE_BITS + SEQUENCE_BITS;
const SCOPE_SHIFT: u64 = TIMESTAMP_BITS + TIMESTAMP_SHIFT;
const SEQUENCE_MASK: u64 = (1 << SEQUENCE_BITS) - 1;

pub type OID = u64;

lazy_static! {
  pub static ref LOCAL_DOC_ID_GEN: Mutex<DocIDGen> = Mutex::new(DocIDGen::new());
}

pub struct DocIDGen {
  node_id: u64,
  sequence: u64,
  last_timestamp: u64,
}

impl Default for DocIDGen {
  fn default() -> Self {
    Self::new()
  }
}

impl DocIDGen {
  #[allow(dead_code)]
  pub fn new() -> DocIDGen {
    DocIDGen {
      node_id: 0,
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

  if_wasm! {
     fn timestamp(&self) -> u64 {
      js_sys::Date::now() as u64
     }
  }

  if_native! {
    fn timestamp(&self) -> u64 {
      std::time::SystemTime::now()
      .duration_since(std::time::SystemTime::UNIX_EPOCH)
      .expect("Clock moved backwards!")
      .as_millis() as u64
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::sync::Arc;
  use std::thread;

  use crate::local_storage::kv::oid::LOCAL_DOC_ID_GEN;
  use parking_lot::RwLock;

  #[test]
  fn test_oid_gen() {
    let map = Arc::new(RwLock::new(HashMap::new()));

    let mut handles = vec![];
    for _i in 0..2 {
      let cloned_map = map.clone();
      let handle = thread::spawn(move || {
        let mut a = LOCAL_DOC_ID_GEN.lock();
        let id = a.next_id();
        // println!("id: {:b}", a.last_timestamp - EPOCH);
        // println!("id: {:b}", id);
        if cloned_map.read().contains_key(&id) {
          panic!("id: {} is duplicated!", id);
        }
        //   |<-7->| <-----------------41--------------->| <--8--> |<----12---->|
        //           101010100000011010101111101100101000
        // 0 1000000 101010100000011010101111101100101000 00000000 000000000000
        cloned_map.write().insert(id, id);
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }
}
