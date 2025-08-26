use std::cmp::Ordering;
use std::ops::Deref;

pub type Timestamp = u64;

/// Last-Writer-Wins (LWW) register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lww<T> {
  pub(crate) timestamp: Timestamp,
  pub(crate) value: T,
}

impl<T> Lww<T>
where
  T: Ord,
{
  pub fn new(timestamp: Timestamp, value: T) -> Self {
    Self { timestamp, value }
  }

  pub fn update(&mut self, value: T) -> bool {
    let timestamp = now();
    self.update_with(timestamp, value)
  }

  pub fn update_with(&mut self, timestamp: Timestamp, value: T) -> bool {
    match self.timestamp.cmp(&timestamp) {
      Ordering::Less => {
        self.timestamp = timestamp;
        self.value = value;
        true
      },
      Ordering::Equal if self.value < value => {
        self.value = value;
        true
      },
      _ => false,
    }
  }

  pub fn merge(&mut self, other: Self) -> bool {
    self.update_with(other.timestamp, other.value)
  }
}

impl<T> Deref for Lww<T> {
  type Target = T;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

pub fn now() -> Timestamp {
  use std::time::{SystemTime, UNIX_EPOCH};
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards")
    .as_millis() as u64
}
