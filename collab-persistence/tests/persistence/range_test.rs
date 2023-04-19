use collab_persistence::keys::{clock_from_key, make_doc_update_key, Clock};
use parking_lot::RwLock;
use smallvec::{smallvec, SmallVec};

use std::io::Write;
use std::ops::{Deref, Range, RangeTo};
use std::sync::Arc;
use std::thread;

use crate::util::db;

#[test]
fn id_test() {
  let db = db().1;
  db.insert([0, 0, 0, 0, 0, 0, 0, 0], &[0, 1, 1]).unwrap();
  db.insert([0, 0, 0, 0, 0, 0, 0, 1], &[0, 1, 2]).unwrap();
  db.insert([0, 0, 0, 0, 0, 0, 0, 2], &[0, 1, 3]).unwrap();
  db.insert([0, 0, 0, 0, 0, 0, 0, 3], &[0, 1, 4]).unwrap();
  db.insert([0, 1, 0, 0, 0, 0, 0, 4], &[0, 1, 5]).unwrap();
  db.insert([0, 1, 0, 0, 0, 0, 0, 5], &[0, 1, 6]).unwrap();

  let given_key: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, 1];
  let last_entry_prior = db
      .range::<&[u8; 8], RangeTo<&[u8; 8]>>(..given_key) // Create a range up to (excluding) the given key
      .next_back()
      .expect("No entry found prior to the given key").unwrap();
  assert_eq!(last_entry_prior.1.as_ref(), &[0, 1, 1]);

  let given_key: &[u8; 2] = &[0, 1];
  let last_entry_prior = db
      .range::<&[u8; 2], RangeTo<&[u8; 2]>>(..given_key) // Create a range up to (excluding) the given key
      .next_back()
      .expect("No entry found prior to the given key").unwrap();
  println!("{:?}", last_entry_prior.1);

  let prefix: &[u8] = &[0, 1, 0, 0, 0, 0, 0];
  let mut r = db.scan_prefix(prefix);
  println!("{:?}", r.next_back())
}

#[test]
fn key_range_test() {
  let db = db().1;
  let next = || {
    let given_key: &[u8; 2] = &[0, 2];
    let val = db
        .range::<&[u8; 2], RangeTo<&[u8; 2]>>(..given_key) // Create a range up to (excluding) the given key
        .next_back()
        .expect("No entry found prior to the given key").unwrap();

    u64::from_be_bytes(val.1.as_ref().try_into().unwrap())
  };

  db.insert([0, 0, 0, 0, 0, 0, 0, 0], &(1 as u64).to_be_bytes())
    .unwrap();
  assert_eq!(next(), 1);

  db.insert([0, 0, 0, 0, 0, 0, 1, 1], &(2 as u64).to_be_bytes())
    .unwrap();
  assert_eq!(next(), 2);

  db.insert([0, 0, 0, 0, 0, 0, 1, 2], &(3 as u64).to_be_bytes())
    .unwrap();
  assert_eq!(next(), 3);

  db.insert([0, 0, 1, 0, 0, 0, 1, 2], &(4 as u64).to_be_bytes())
    .unwrap();
  assert_eq!(next(), 4);
}

#[test]
fn scan_prefix() {
  let db = db().1;
  let doc_id: i64 = 1;
  let mut v: SmallVec<[u8; 12]> = smallvec![1, 1];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(255);
  assert_eq!(v.as_ref(), &[1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 255]);

  db.insert(v.as_ref(), &[0, 1, 1]).unwrap();
  let val = db.scan_prefix(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 2]).last();
  assert!(val.is_none());
}

#[test]
fn scan_prefix_multi_thread() {
  let db = Arc::new(RwLock::new(db().1));
  let mut handles = vec![];
  let doc_id: u64 = 1;

  for i in 0..1000 {
    let step: i64 = i;
    let cloned_db = db.clone();
    let update_data = i.to_be_bytes();

    let handle = thread::spawn(move || {
      let cloned_db = cloned_db.write();
      {
        println!("start: {}", step);
        let max_key = make_doc_update_key(doc_id, Clock::MAX);
        let last_clock = if let Some(Ok((k, _v))) = cloned_db.range(..max_key).next_back() {
          let clock_byte = clock_from_key(k.as_ref());
          Clock::from_be_bytes(clock_byte.try_into().unwrap())
        } else {
          0
        };

        let clock = last_clock + 1;
        let new_key = make_doc_update_key(doc_id, clock);
        println!("value: {}", clock);
        cloned_db.insert(new_key.as_ref(), &update_data).unwrap();
        println!("stop: {}", step);
        println!("*****");
      }
      drop(cloned_db);
    });

    handles.push(handle);
  }
  for handle in handles {
    handle.join().unwrap();
  }
}

#[test]
fn range_key_test() {
  let db = db().1;
  db.insert([0, 0, 0, 0, 0, 0, 0, 0], &[0, 1, 1]).unwrap();
  db.insert([0, 0, 0, 0, 0, 0, 0, 1], &[0, 1, 2]).unwrap();
  db.insert([0, 0, 0, 0, 0, 0, 0, 2], &[0, 1, 3]).unwrap();

  db.insert([0, 0, 1, 0, 0, 0, 0, 0], &[0, 2, 1]).unwrap();
  db.insert([0, 0, 1, 0, 0, 0, 0, 1], &[0, 2, 2]).unwrap();
  db.insert([0, 0, 1, 0, 0, 0, 0, 2], &[0, 2, 3]).unwrap();

  db.insert([0, 0, 2, 0, 0, 0, 0, 0], &[0, 3, 1]).unwrap();
  db.insert([0, 0, 2, 0, 0, 0, 0, 1], &[0, 3, 2]).unwrap();
  db.insert([0, 0, 2, 0, 0, 0, 0, 2], &[0, 3, 3]).unwrap();

  db.insert([0, 1, 0, 0, 0, 0, 0, 3], &[0, 1, 4]).unwrap();
  db.insert([0, 1, 0, 0, 0, 0, 0, 4], &[0, 1, 5]).unwrap();
  db.insert([0, 1, 0, 0, 0, 0, 0, 5], &[0, 1, 6]).unwrap();

  let given_key: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, u8::MAX];
  let mut iter = db.range::<&[u8; 8], RangeTo<&[u8; 8]>>(..given_key);
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 0, 0, 0, 0, 0, 0]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 0, 0, 0, 0, 0, 1]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 0, 0, 0, 0, 0, 2]
  );
  assert!(iter.next().is_none());

  let start: &[u8; 8] = &[0, 0, 1, 0, 0, 0, 0, 0];
  let given_key: &[u8; 8] = &[0, 0, 1, 0, 0, 0, 0, u8::MAX];
  let mut iter = db.range::<&[u8; 8], Range<&[u8; 8]>>(start..given_key);
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 1, 0, 0, 0, 0, 0]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 1, 0, 0, 0, 0, 1]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 0, 1, 0, 0, 0, 0, 2]
  );
  assert!(iter.next().is_none());

  let given_key: &[u8; 2] = &[0, 1];
  let last_entry_prior = db
      .range::<&[u8; 2], RangeTo<&[u8; 2]>>(..given_key) // Create a range up to (excluding) the given key
      .next_back()
      .expect("No entry found prior to the given key").unwrap();
  assert_eq!(last_entry_prior.1.as_ref(), &[0, 3, 3]);

  let prefix: &[u8] = &[0, 0, 2, 0, 0, 0, 0];
  let r = db.scan_prefix(prefix);
  assert_eq!(
    r.last().unwrap().unwrap().0.as_ref(),
    &[0, 0, 2, 0, 0, 0, 0, 2]
  );

  let start: &[u8; 8] = &[0, 1, 0, 0, 0, 0, 0, 3];
  let given_key: &[u8; 8] = &[0, 1, 0, 0, 0, 0, 0, u8::MAX];
  let mut iter = db.range::<&[u8; 8], Range<&[u8; 8]>>(start..given_key);
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 1, 0, 0, 0, 0, 0, 3]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 1, 0, 0, 0, 0, 0, 4]
  );
  assert_eq!(
    iter.next().unwrap().unwrap().0.as_ref(),
    &[0, 1, 0, 0, 0, 0, 0, 5]
  );
  assert!(iter.next().is_none());
}

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key<const N: usize>(pub SmallVec<[u8; N]>);

impl<const N: usize> Key<N> {
  pub const fn from_const(src: [u8; N]) -> Self {
    Key(SmallVec::from_const(src))
  }
}

impl<const N: usize> Deref for Key<N> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.0.as_ref()
  }
}

impl<const N: usize> AsRef<[u8]> for Key<N> {
  #[inline]
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl<const N: usize> AsMut<[u8]> for Key<N> {
  #[inline]
  fn as_mut(&mut self) -> &mut [u8] {
    self.0.as_mut()
  }
}

impl<const N: usize> From<Key<N>> for Vec<u8> {
  fn from(key: Key<N>) -> Self {
    key.0.to_vec()
  }
}
