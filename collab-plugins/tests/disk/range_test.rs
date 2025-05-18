use std::ops::{Deref, Range, RangeTo};
use std::sync::Arc;
use std::thread;

use crate::disk::util::rocks_db;
use collab_plugins::local_storage::kv::keys::{Clock, clock_from_key, make_doc_update_key};
use collab_plugins::local_storage::kv::{KVEntry, KVStore, KVTransactionDB};
use smallvec::SmallVec;

#[tokio::test]
async fn rocks_id_test() {
  let rocks_db = rocks_db().1;
  rocks_db
    .with_write_txn(|store| {
      store.insert([0, 0, 0, 0, 0, 0, 0, 0], [0, 1, 1]).unwrap();
      store.insert([0, 0, 0, 0, 0, 0, 0, 1], [0, 1, 2]).unwrap();
      store.insert([0, 0, 0, 0, 0, 0, 0, 2], [0, 1, 3]).unwrap();
      store.insert([0, 0, 0, 0, 0, 0, 0, 3], [0, 1, 4]).unwrap();
      store.insert([0, 1, 0, 0, 0, 0, 0, 4], [0, 1, 5]).unwrap();
      store.insert([0, 1, 0, 0, 0, 0, 0, 5], [0, 1, 6]).unwrap();
      Ok(())
    })
    .unwrap();

  let given_key: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, 1];
  let last_entry_prior = rocks_db
    .read_txn()
    .next_back_entry(given_key)
    .expect("No entry found prior to the given key")
    .unwrap();
  assert_eq!(last_entry_prior.value(), &[0, 1, 2]);

  let given_key: &[u8; 2] = &[0, 1];
  let last_entry_prior = rocks_db
    .read_txn()
    .next_back_entry(given_key)
    .expect("No entry found prior to the given key")
    .unwrap();
  println!("{:?}", last_entry_prior.value());

  // The start key is exclusive
  let txn = rocks_db.read_txn();
  let mut range = txn
    .range([0, 0, 0, 0, 0, 0, 0, 0]..[0, 0, 0, 0, 0, 0, 0, 2])
    .unwrap();
  assert_eq!(range.next().unwrap().value(), &[0, 1, 1]);
  assert_eq!(range.next().unwrap().value(), &[0, 1, 2]);
  assert!(range.next().is_none());

  // The end key is exclusive
  rocks_db
    .with_write_txn(|txn| {
      txn
        .remove_range(&[0, 0, 0, 0, 0, 0, 0, 0], &[0, 0, 0, 0, 0, 0, 0, 2])
        .unwrap();
      Ok(())
    })
    .unwrap();

  let txn = rocks_db.read_txn();
  let value = txn.get([0, 0, 0, 0, 0, 0, 0, 2]).unwrap().unwrap();
  assert_eq!(&value, &[0, 1, 3]);
}

#[tokio::test]
async fn key_range_test() {
  let db = rocks_db().1;
  let next = || {
    let given_key: &[u8; 2] = &[0, 2];
    let val = db
      .read_txn()
      .next_back_entry(given_key)
      .expect("No entry found prior to the given key")
      .unwrap();

    u64::from_be_bytes(val.value().try_into().unwrap())
  };

  db.with_write_txn(|w| w.insert([0, 0, 0, 0, 0, 0, 0, 0], 1_u64.to_be_bytes()))
    .unwrap();

  assert_eq!(next(), 1);

  db.with_write_txn(|w| w.insert([0, 0, 0, 0, 0, 0, 1, 1], 2_u64.to_be_bytes()))
    .unwrap();
  assert_eq!(next(), 2);

  db.with_write_txn(|w| w.insert([0, 0, 0, 0, 0, 0, 1, 2], 3_u64.to_be_bytes()))
    .unwrap();
  assert_eq!(next(), 3);

  db.with_write_txn(|w| w.insert([0, 0, 1, 0, 0, 0, 1, 2], 4_u64.to_be_bytes()))
    .unwrap();
  assert_eq!(next(), 4);
}

#[tokio::test]
async fn scan_prefix_multi_thread() {
  let db = Arc::new(rocks_db().1);
  let mut handles = vec![];
  let doc_id: u64 = 1;

  for i in 0..1000 {
    let step: i64 = i;
    let cloned_db = db.clone();
    let update_data = i.to_be_bytes();

    let handle = thread::spawn(move || {
      let cloned_db = cloned_db.read_txn();
      {
        //println!("start: {}", step);
        let max_key = make_doc_update_key(doc_id, Clock::MAX);
        let last_clock = if let Ok(Some(entry)) = cloned_db.next_back_entry(max_key.as_ref()) {
          let clock_byte = clock_from_key(entry.key());
          Clock::from_be_bytes(clock_byte.try_into().unwrap())
        } else {
          0
        };

        let clock = last_clock + 1;
        let new_key = make_doc_update_key(doc_id, clock);
        println!("value: {}", clock);
        cloned_db.insert(new_key.as_ref(), update_data).unwrap();
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

#[tokio::test]
async fn range_key_test() {
  let db = rocks_db().1;
  db.with_write_txn(|store| {
    store.insert([0, 0, 0, 0, 0, 0, 0, 0], [0, 1, 1]).unwrap();
    store.insert([0, 0, 0, 0, 0, 0, 0, 1], [0, 1, 2]).unwrap();
    store.insert([0, 0, 0, 0, 0, 0, 0, 2], [0, 1, 3]).unwrap();

    store.insert([0, 0, 1, 0, 0, 0, 0, 0], [0, 2, 1]).unwrap();
    store.insert([0, 0, 1, 0, 0, 0, 0, 1], [0, 2, 2]).unwrap();
    store.insert([0, 0, 1, 0, 0, 0, 0, 2], [0, 2, 3]).unwrap();

    store.insert([0, 0, 2, 0, 0, 0, 0, 0], [0, 3, 1]).unwrap();
    store.insert([0, 0, 2, 0, 0, 0, 0, 1], [0, 3, 2]).unwrap();
    store.insert([0, 0, 2, 0, 0, 0, 0, 2], [0, 3, 3]).unwrap();

    store.insert([0, 1, 0, 0, 0, 0, 0, 3], [0, 1, 4]).unwrap();
    store.insert([0, 1, 0, 0, 0, 0, 0, 4], [0, 1, 5]).unwrap();
    store.insert([0, 1, 0, 0, 0, 0, 0, 5], [0, 1, 6]).unwrap();
    Ok(())
  })
  .unwrap();

  let given_key: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, u8::MAX];
  let store = db.read_txn();
  let mut iter = store
    .range::<&[u8; 8], RangeTo<&[u8; 8]>>(..given_key)
    .unwrap();
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 1]);
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 2]);
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 3]);
  assert!(iter.next().is_none());

  let start: &[u8; 8] = &[0, 0, 1, 0, 0, 0, 0, 0];
  let given_key: &[u8; 8] = &[0, 0, 1, 0, 0, 0, 0, u8::MAX];
  let mut iter = store
    .range::<&[u8; 8], Range<&[u8; 8]>>(start..given_key)
    .unwrap();
  assert_eq!(iter.next().unwrap().value(), &[0, 2, 1]);
  assert_eq!(iter.next().unwrap().value(), &[0, 2, 2]);
  assert_eq!(iter.next().unwrap().value(), &[0, 2, 3]);
  assert!(iter.next().is_none());

  let given_key: &[u8; 2] = &[0, 1];
  let last_entry_prior = store
      .next_back_entry(given_key) // Create a range up to (excluding) the given key
      .expect("No entry found prior to the given key").unwrap();
  assert_eq!(last_entry_prior.value(), &[0, 3, 3]);

  let start: &[u8; 8] = &[0, 1, 0, 0, 0, 0, 0, 3];
  let given_key: &[u8; 8] = &[0, 1, 0, 0, 0, 0, 0, u8::MAX];
  let mut iter = store
    .range::<&[u8; 8], Range<&[u8; 8]>>(start..given_key)
    .unwrap();
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 4]);
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 5]);
  assert_eq!(iter.next().unwrap().value(), &[0, 1, 6]);
  assert!(iter.next().is_none());
}

#[tokio::test]
async fn delete_range_test() {
  let db = rocks_db().1;
  db.with_write_txn(|store| {
    store.insert([0, 0, 0, 0, 0, 0, 0, 0], [0, 1, 1]).unwrap();
    store.insert([0, 0, 0, 0, 0, 0, 0, 1], [0, 1, 2]).unwrap();
    store.insert([0, 0, 0, 0, 0, 0, 0, 2], [0, 1, 3]).unwrap();

    store.insert([0, 0, 1, 0, 0, 0, 0, 0], [0, 2, 1]).unwrap();
    store.insert([0, 0, 1, 0, 0, 0, 0, 1], [0, 2, 2]).unwrap();
    store.insert([0, 0, 1, 0, 0, 0, 0, 2], [0, 2, 3]).unwrap();

    Ok(())
  })
  .unwrap();

  let given_key: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, u8::MAX];
  let store = db.read_txn();
  let iter = store
    .range::<&[u8; 8], RangeTo<&[u8; 8]>>(..given_key)
    .unwrap();
  assert_eq!(iter.count(), 3);

  // remove the keys
  db.with_write_txn(|write_txn| {
    write_txn
      .remove_range(&[0, 0, 0, 0, 0, 0, 0, 0], given_key)
      .unwrap();
    Ok(())
  })
  .unwrap();

  // check that the keys are removed
  let store = db.read_txn();
  let iter = store
    .range::<&[u8; 8], RangeTo<&[u8; 8]>>(..given_key)
    .unwrap();
  assert_eq!(iter.count(), 0);
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
