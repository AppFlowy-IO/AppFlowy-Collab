use std::ops::{Range, RangeInclusive, RangeToInclusive};

#[derive(Clone)]
pub struct CLRange {
  pub(crate) start: i64,
  pub(crate) end: i64,
}

impl CLRange {
  /// Construct a new `RevRange` representing the range [start..end).
  /// It is an invariant that `start <= end`.
  pub fn new(start: i64, end: i64) -> CLRange {
    debug_assert!(start <= end);
    CLRange { start, end }
  }
}

impl From<RangeInclusive<i64>> for CLRange {
  fn from(src: RangeInclusive<i64>) -> CLRange {
    CLRange::new(*src.start(), src.end().saturating_add(1))
  }
}

impl From<RangeToInclusive<i64>> for CLRange {
  fn from(src: RangeToInclusive<i64>) -> CLRange {
    CLRange::new(0, src.end.saturating_add(1))
  }
}

impl From<Range<i64>> for CLRange {
  fn from(src: Range<i64>) -> CLRange {
    let Range { start, end } = src;
    CLRange { start, end }
  }
}

impl Iterator for CLRange {
  type Item = i64;

  fn next(&mut self) -> Option<i64> {
    if self.start > self.end {
      return None;
    }
    let val = self.start;
    self.start += 1;
    Some(val)
  }
}
