use std::time::SystemTime;

/// Equivalent to April 9, 2023 4:18:02 AM
const EPOCH: u64 = 1637806706000;
const NODE_BITS: u64 = 10;
/// 14 bits. For every ID generated on that machine/process, the sequence number is incremented by 1.
/// The number is reset to 0 every millisecond.
const SEQUENCE_BITS: u64 = 12;
const NODE_ID_SHIFT: u64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u64 = NODE_BITS + SEQUENCE_BITS;

/// SEQUENCE_MASK is a u64 integer representing a bitmask with the value 4095 (in binary, 111111111111).
/// This mask can be used in bitwise operations (e.g., AND &, OR |, XOR ^) to manipulate or extract
/// the least significant 12 bits of other u64 integers.
const SEQUENCE_MASK: u64 = (1 << SEQUENCE_BITS) - 1;

pub struct DatabaseIDGen {
  node_id: u64,
  sequence: u64,
  last_timestamp: u64,
}

impl DatabaseIDGen {
  pub fn new(node_id: u64) -> DatabaseIDGen {
    DatabaseIDGen {
      node_id,
      sequence: 0,
      last_timestamp: 0,
    }
  }

  pub fn next_id(&mut self) -> i64 {
    let timestamp = self.timestamp();
    if timestamp < self.last_timestamp {
      panic!("Clock moved backwards!");
    }

    if timestamp == self.last_timestamp {
      // use the bitwise AND & operator with SEQUENCE_MASK to extract the least significant 12 bits
      self.sequence = (self.sequence + 1) & SEQUENCE_MASK;
      if self.sequence == 0 {
        self.wait_next_millis();
      }
    } else {
      self.sequence = 0;
    }

    self.last_timestamp = timestamp;
    let id = (timestamp - EPOCH) << TIMESTAMP_SHIFT | self.node_id << NODE_ID_SHIFT | self.sequence;
    id as i64
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

#[cfg(test)]
mod tests {}
