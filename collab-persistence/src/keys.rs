use smallvec::{smallvec, SmallVec};
use std::io::Write;
use std::ops::Deref;

// https://github.com/spacejam/sled
// sled performs prefix encoding on long keys with similar prefixes that are grouped together in a
// range, as well as suffix truncation to further reduce the indexing costs of long keys. Nodes
// will skip potentially expensive length and offset pointers if keys or values are all the same
// length (tracked separately, don't worry about making keys the same length as values), so it
// may improve space usage slightly if you use fixed-length keys or values. This also makes it
// easier to use structured access as well.
//
// DOC_SPACE
//     DOC_SPACE_OBJECT       object_id   TERMINATOR
//     DOC_SPACE_OBJECT_KEY     doc_id      DOC_STATE (state start)
//     DOC_SPACE_OBJECT_KEY     doc_id      TERMINATOR_HI_WATERMARK (state end)
//     DOC_SPACE_OBJECT_KEY     doc_id      DOC_STATE_VEC (state vector)
//     DOC_SPACE_OBJECT_KEY     doc_id      DOC_UPDATE clock TERMINATOR (update)
//
// SNAPSHOT_SPACE
//     SNAPSHOT_SPACE_OBJECT        object_id       TERMINATOR
//     SNAPSHOT_SPACE_OBJECT_KEY    snapshot_id     SNAPSHOT_UPDATE(snapshot)

/// Prefix byte used for all of the yrs object entries.
pub const DOC_SPACE: u8 = 0;

/// Prefix byte used for object id -> [DocID] mapping index key space.
pub const DOC_SPACE_OBJECT: u8 = 0;
/// Prefix byte used for object key space.
pub const DOC_SPACE_OBJECT_KEY: u8 = 1;

pub const TERMINATOR: u8 = 0;

pub const TERMINATOR_HI_WATERMARK: u8 = 255;

/// Tag byte within [DOC_SPACE_OBJECT_KEY] used to identify object's state entry.
pub const DOC_STATE: u8 = 0;

/// Tag byte within [DOC_SPACE_OBJECT_KEY] used to identify object's state vector entry.
pub const DOC_STATE_VEC: u8 = 1;

/// Tag byte within [DOC_SPACE_OBJECT_KEY] used to identify object's update entries.
pub const DOC_UPDATE: u8 = 2;

/// Prefix byte used for snapshot id -> [SnapshotID] mapping index key space.
pub const SNAPSHOT_SPACE: u8 = 1;

/// Prefix byte used for snapshot key space.
pub const SNAPSHOT_SPACE_OBJECT: u8 = 0;

/// Tag byte within [SNAPSHOT_SPACE_OBJECT] used to identify object's snapshot entries.
pub const SNAPSHOT_UPDATE: u8 = 1;

pub type DocID = u32;
pub type SnapshotID = u32;

pub fn make_doc_id(object_id: &[u8]) -> Key<20> {
  let mut v: SmallVec<[u8; 20]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT];
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn doc_name_from_key(key: &[u8]) -> &[u8] {
  &key[2..(key.len() - 1)]
}

pub fn make_doc_state_key(doc_id: DocID) -> Key<8> {
  let mut v: SmallVec<[u8; 8]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_STATE);
  Key(v)
}

// document related elements are stored within bounds [0,1,..did,0]..[0,1,..did,255]
pub fn make_doc_start_key(doc_id: DocID) -> Key<8> {
  make_doc_state_key(doc_id)
}

pub fn make_doc_end_key(doc_id: DocID) -> Key<8> {
  let mut v: SmallVec<[u8; 8]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(TERMINATOR_HI_WATERMARK);
  Key(v)
}

pub fn make_state_vector_key(doc_id: DocID) -> Key<8> {
  let mut v: SmallVec<[u8; 8]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_STATE_VEC);
  Key(v)
}

pub fn make_update_key(doc_id: DocID, clock: u32) -> Key<12> {
  let mut v: SmallVec<[u8; 12]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_UPDATE);
  v.write_all(&clock.to_be_bytes()).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn clock_from_key(key: &[u8]) -> &[u8] {
  let len = key.len();
  // update key scheme: 01{name:n}1{clock:4}0
  &key[(len - 5)..(len - 1)]
}

pub fn make_snapshot_id(object_id: &[u8]) -> Key<20> {
  let mut v: SmallVec<[u8; 20]> = smallvec![SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT];
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn make_snapshot_key(snapshot_id: SnapshotID, clock: u32) -> Key<12> {
  let mut v: SmallVec<[u8; 12]> = smallvec![SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT];
  v.write_all(&snapshot_id.to_be_bytes()).unwrap();
  v.push(SNAPSHOT_UPDATE);
  v.write_all(&clock.to_be_bytes()).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key<const N: usize>(SmallVec<[u8; N]>);

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
