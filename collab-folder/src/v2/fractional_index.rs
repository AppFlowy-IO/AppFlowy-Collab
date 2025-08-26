use std::collections::BTreeMap;

pub type FractionalIndex = String;
pub type FractionalVec<T> = BTreeMap<FractionalIndex, T>;

pub fn neighbors<T>(
  map: &FractionalVec<T>,
  index: Option<usize>,
) -> (Option<&FractionalIndex>, Option<&FractionalIndex>) {
  match index {
    None => {
      let left = map.last_key_value().map(|(k, _)| k);
      (left, None)
    },
    Some(mut i) => {
      let mut left = None;
      let mut right = None;
      let mut iter = map.keys();
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

pub fn neighbors_after<T, F>(
  map: &FractionalVec<T>,
  predicate: F,
) -> (Option<&FractionalIndex>, Option<&FractionalIndex>)
where
  F: Fn(&T) -> bool,
{
  let mut left_index = None;
  let mut right_index = None;
  let mut i = map.iter();
  while let Some((index, item)) = i.next() {
    if predicate(item) {
      left_index = Some(index);
      right_index = i.next().map(|(index, _)| index);
      break;
    }
  }
  if left_index.is_none() {
    right_index = map.first_key_value().map(|(i, _)| i);
  }
  (left_index, right_index)
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
