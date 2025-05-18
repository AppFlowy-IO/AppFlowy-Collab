use std::io::Write;
use std::ops::Deref;

use smallvec::{SmallVec, smallvec};

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
pub const DOC_SPACE: u8 = 1;

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

pub const REMOTE_DOC_STATE_VEC: u8 = 2;

/// Tag byte within [DOC_SPACE_OBJECT_KEY] used to identify object's update entries.
pub const DOC_UPDATE: u8 = 2;

/// Prefix byte used for snapshot id -> [SnapshotID] mapping index key space.
pub const SNAPSHOT_SPACE: u8 = 2;

/// Prefix byte used for snapshot key space.
pub const SNAPSHOT_SPACE_OBJECT: u8 = 0;

/// Tag byte within [SNAPSHOT_SPACE_OBJECT] used to identify object's snapshot entries.
pub const SNAPSHOT_UPDATE: u8 = 1;

pub const COLLAB_SPACE: u8 = 3;
pub const COLLAB_SPACE_OBJECT: u8 = 0;

pub type DocID = u64;
pub const DOC_ID_LEN: usize = 8;
pub const DOC_STATE_KEY_LEN: usize = DOC_ID_LEN + 4;
pub const DOC_UPDATE_KEY_LEN: usize = DOC_ID_LEN + CLOCK_LEN + 4;
pub const DOC_UPDATE_KEY_PREFIX_LEN: usize = DOC_ID_LEN + 4;

pub type SnapshotID = u64;
pub const SNAPSHOT_ID_LEN: usize = 8;
pub const SNAPSHOT_UPDATE_KEY_LEN: usize = SNAPSHOT_ID_LEN + CLOCK_LEN + 4;
pub const SNAPSHOT_UPDATE_KEY_PREFIX_LEN: usize = SNAPSHOT_ID_LEN + 4;

pub type Clock = u32;
pub const CLOCK_LEN: usize = 4;

pub fn make_doc_id_key_v1(uid: &[u8], workspace_id: &[u8], object_id: &[u8]) -> Key<20> {
  // uuid: 16 bytes
  // uid: 8 bytes
  // 16 * 2 + 8 + 2+ 1 = 43
  let mut v: SmallVec<[u8; 20]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT];
  v.write_all(uid).unwrap();
  v.write_all(workspace_id).unwrap();
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn make_doc_id_key_v0(uid: &[u8], object_id: &[u8]) -> Key<20> {
  let mut v: SmallVec<[u8; 20]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT];
  v.write_all(uid).unwrap();
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn oid_from_key(key: &[u8]) -> &[u8] {
  // [DOC_SPACE, DOC_SPACE_OBJECT] = 2
  // uid = 8
  // 2 + 8 = 10
  // TERMINATOR = 1
  &key[10..(key.len() - 1)]
}

// [1,1,  0,0,0,0,0,0,0,0,  0]
pub fn make_doc_state_key(doc_id: DocID) -> Key<DOC_STATE_KEY_LEN> {
  let mut v: SmallVec<[u8; DOC_STATE_KEY_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_STATE);
  Key(v)
}

// document related elements are stored within bounds [0,1,..did,0]..[0,1,..did,255]
pub fn make_doc_start_key(doc_id: DocID) -> Key<DOC_STATE_KEY_LEN> {
  make_doc_state_key(doc_id)
}
// [1,1,  0,0,0,0,0,0,0,0,  255]
pub fn make_doc_end_key(doc_id: DocID) -> Key<DOC_STATE_KEY_LEN> {
  let mut v: SmallVec<[u8; DOC_STATE_KEY_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(TERMINATOR_HI_WATERMARK);
  Key(v)
}

// [1,1,  0,0,0,0,0,0,0,0,  1]
pub fn make_state_vector_key(doc_id: DocID) -> Key<DOC_STATE_KEY_LEN> {
  let mut v: SmallVec<[u8; DOC_STATE_KEY_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_STATE_VEC);
  Key(v)
}

// [1,1,  0,0,0,0,0,0,0,0,  2]
pub fn make_remote_state_vector_key(doc_id: DocID) -> Key<DOC_STATE_KEY_LEN> {
  let mut v: SmallVec<[u8; DOC_STATE_KEY_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(REMOTE_DOC_STATE_VEC);
  Key(v)
}

// [1,1,  0,0,0,0,0,0,0,0,  2   0,0,0,0,  0]
pub fn make_doc_update_key(doc_id: DocID, clock: Clock) -> Key<DOC_UPDATE_KEY_LEN> {
  let mut v: SmallVec<[u8; DOC_UPDATE_KEY_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_UPDATE);
  v.write_all(&clock.to_be_bytes()).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

// [1,1,  0,0,0,0,0,0,0,0,  2]
pub fn make_doc_update_key_prefix(doc_id: DocID) -> Key<DOC_UPDATE_KEY_PREFIX_LEN> {
  let mut v: SmallVec<[u8; DOC_UPDATE_KEY_PREFIX_LEN]> = smallvec![DOC_SPACE, DOC_SPACE_OBJECT_KEY];
  v.write_all(&doc_id.to_be_bytes()).unwrap();
  v.push(DOC_UPDATE);
  Key(v)
}

// [1,1,  0,0,0,0,0,0,0,0,  2   [0,0,0,0],  0]
pub fn clock_from_key(key: &[u8]) -> &[u8] {
  let len = key.len();
  &key[(len - 5)..(len - 1)]
}

// [10,0, uid,  object_id,  0]
pub fn make_snapshot_id_key(uid: &[u8], object_id: &[u8]) -> Key<20> {
  let mut v: SmallVec<[u8; 20]> = smallvec![SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT];
  v.write_all(uid).unwrap();
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

// [10,0,  0,0,0,0,0,0,0,0,  1   [0,0,0,0],  0]
pub fn make_snapshot_update_key(
  snapshot_id: SnapshotID,
  clock: Clock,
) -> Key<SNAPSHOT_UPDATE_KEY_LEN> {
  let mut v: SmallVec<[u8; SNAPSHOT_UPDATE_KEY_LEN]> =
    smallvec![SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT];
  v.write_all(&snapshot_id.to_be_bytes()).unwrap();
  v.push(SNAPSHOT_UPDATE);
  v.write_all(&clock.to_be_bytes()).unwrap();
  v.push(TERMINATOR);
  Key(v)
}

pub fn make_snapshot_update_key_prefix(
  snapshot_id: SnapshotID,
) -> Key<SNAPSHOT_UPDATE_KEY_PREFIX_LEN> {
  let mut v: SmallVec<[u8; SNAPSHOT_UPDATE_KEY_PREFIX_LEN]> =
    smallvec![SNAPSHOT_SPACE, SNAPSHOT_SPACE_OBJECT];
  v.write_all(&snapshot_id.to_be_bytes()).unwrap();
  v.push(SNAPSHOT_UPDATE);
  Key(v)
}

pub fn make_collab_id_key(object_id: &[u8]) -> Key<20> {
  let mut v: SmallVec<[u8; 20]> = smallvec![COLLAB_SPACE, COLLAB_SPACE_OBJECT];
  v.write_all(object_id).unwrap();
  v.push(TERMINATOR);
  Key(v)
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
