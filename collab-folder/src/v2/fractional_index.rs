use crate::SectionItem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type FractionalIndex = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FractionalVec<T>(BTreeMap<FractionalIndex, Option<T>>);

impl<T> FractionalVec<T> {
  pub fn insert(&mut self, index: usize, value: T) {
    assert!(index < self.0.len());
    let new_index = self.index_at(Some(index));
    self.0.insert(new_index, Some(value));
  }

  pub fn append(&mut self, items: impl Iterator<Item = T>) {
    let mut last_index = self.0.keys().last().cloned();
    for item in items {
      let index = index_between(last_index.as_ref(), None).unwrap();
      self.0.insert(index.clone(), Some(item));
      last_index = Some(index);
    }
  }

  pub fn index_at(&self, index: Option<usize>) -> FractionalIndex {
    let (left, right) = self.neighbors(index);
    index_between(left, right).expect("Failed to create a new index")
  }

  pub fn insert_after<F>(&mut self, value: T, predicate: F)
  where
    F: Fn(&T) -> bool,
  {
    let (left, right) = self.neighbors_after(predicate);
    let index = index_between(left, right).expect("Failed to create a new index");
    self.0.insert(index, Some(value));
  }

  pub fn iter(&self) -> impl Iterator<Item = &T> {
    self.0.values().flat_map(Option::as_ref)
  }

  pub fn keys(&self) -> impl Iterator<Item = &FractionalIndex> {
    self.0.keys()
  }

  fn neighbors(
    &self,
    index: Option<usize>,
  ) -> (Option<&FractionalIndex>, Option<&FractionalIndex>) {
    match index {
      None => {
        let left = self.0.last_key_value().map(|(k, _)| k);
        (left, None)
      },
      Some(mut i) => {
        let mut left = None;
        let mut right = None;
        let mut iter = self.0.keys();
        while let Some(key) = iter.next() {
          if i == 0 {
            right = Some(key);
            break;
          }
          left = Some(key);
          i -= 1;
        }
        (left, right)
      },
    }
  }

  pub fn neighbors_after<F>(
    &self,
    predicate: F,
  ) -> (Option<&FractionalIndex>, Option<&FractionalIndex>)
  where
    F: Fn(&T) -> bool,
  {
    let mut left_index = None;
    let mut right_index = None;
    let mut i = self.0.iter();
    while let Some((index, item)) = i.next() {
      if let Some(item) = item {
        if predicate(item) {
          left_index = Some(index);
          right_index = i.next().map(|(index, _)| index);
          break;
        }
      }
    }
    if left_index.is_none() {
      right_index = self.0.first_key_value().map(|(i, _)| i);
    }
    (left_index, right_index)
  }

  pub fn remove_all<F>(&mut self, predicate: F)
  where
    F: Fn(&T) -> bool,
  {
    for (_, v) in self.0.iter_mut() {
      if let Some(value) = v {
        if predicate(value) {
          *v = None;
        }
      }
    }
  }
}

impl<T: PartialEq> FractionalVec<T> {
  pub fn contains(&self, value: &T) -> bool {
    self.iter().any(|v| v == value)
  }
}

impl<T> Default for FractionalVec<T> {
  fn default() -> Self {
    FractionalVec(BTreeMap::new())
  }
}

impl<T> FromIterator<T> for FractionalVec<T> {
  fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
    let mut vec = FractionalVec::default();
    let mut left = None;
    for item in iter {
      let new_index = index_between(left.as_ref(), None).expect("Failed to create a new index");
      vec.0.insert(new_index.clone(), Some(item));
      left = Some(new_index);
    }
    vec
  }
}

impl<T> FromIterator<(FractionalIndex, T)> for FractionalVec<T> {
  fn from_iter<I: IntoIterator<Item = (FractionalIndex, T)>>(iter: I) -> Self {
    FractionalVec(iter.into_iter().map(|(k, v)| (k, Some(v))).collect())
  }
}

/// Creates an index string that is alphabetically between the `left` and `right` indices.
/// It can be used to concurrently insert new items in a sorted list without conflicts, i.e. when
/// adding a sub-view as a child to a parent view at specific index.
///
/// Both input and output strings are using digits `0-9`, uppercase letters `A-Z`, and lowercase letters `a-z`.
/// Using other characters will result in returning `None`.
pub fn index_between(
  left: Option<&FractionalIndex>,
  right: Option<&FractionalIndex>,
) -> Option<String> {
  fn encode(value: u8) -> u8 {
    if value < 10 {
      value + b'0'
    } else if value < 36 {
      value - 10 + b'A'
    } else {
      value - 36 + b'a'
    }
  }

  fn decode(value: u8) -> u8 {
    if value >= b'0' && value <= b'9' {
      value - b'0'
    } else if value >= b'A' && value <= b'Z' {
      value - b'A' + 10
    } else if value >= b'a' && value <= b'z' {
      value - b'a' + 36
    } else {
      panic!("Invalid character for base64 encoding: {}", value as char);
    }
  }

  const MIN_BYTE: u8 = b'0';
  const MAX_BYTE: u8 = b'z';
  let left: &[u8] = left.map(|v| v.as_bytes()).unwrap_or_default();
  let right: &[u8] = right.map(|v| v.as_bytes()).unwrap_or_default();

  let mut i = 0;
  let mut res = Vec::new();
  loop {
    let min = decode(left.get(i).cloned().unwrap_or(MIN_BYTE));
    let max = decode(right.get(i).cloned().unwrap_or(MAX_BYTE));
    if min + 1 < max {
      res.push(encode(min + 1));
      break;
    } else {
      res.push(encode(min));
    }
    i += 1;
  }

  String::from_utf8(res).ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_create_index_between() {
    // there's no space between `a1b2` and `a1b3`, so we need to add extra character
    let left = "a1b2".to_string();
    let right = "a1b3".to_string();
    let result = index_between(Some(&left), Some(&right));
    assert_eq!(result, Some("a1b21".to_string()));

    // there's a space between `a1b2` and `a1b4` for `a1b3`
    let left = "a1b2".to_string();
    let right = "a1b4".to_string();
    let result = index_between(Some(&left), Some(&right));
    assert_eq!(result, Some("a1b3".to_string()));

    // since left is empty, index can be the smallest possible value (here '1')
    let right = "a1b4".to_string();
    let result = index_between(None, Some(&right));
    assert_eq!(result, Some("1".to_string()));

    // since right is empty, any shortest string higher than left is valid
    let left = "a1b4".to_string();
    let result = index_between(Some(&left), None);
    assert_eq!(result, Some("b".to_string()));
  }
}
