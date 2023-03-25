use smallvec::{smallvec, SmallVec};
use std::io::Write;
use std::ops::Deref;
// Optimize your data layout: Sled's B-Tree implementation works best when the keys are sequential,
// so try to organize the data in a way that maximizes sequential access.
/// Prefix byte used for all of the yrs-kvstore entries.
pub const SPACE: u8 = 0;
/// Prefix byte used for document name -> DID mapping index key space.
pub const DID_SPACE: u8 = 0;
/// Prefix byte used for document key space.
pub const DOC_SPACE: u8 = 1;

pub const TERMINATOR: u8 = 0;

pub const TERMINATOR_HI_WATERMARK: u8 = 255;

/// Tag byte within [DOC_SPACE] used to identify document's state entry.
pub const DOC_STATE: u8 = 0;

/// Tag byte within [DOC_SPACE] used to identify document's state vector entry.
pub const DOC_STATE_VEC: u8 = 1;

/// Tag byte within [DOC_SPACE] used to identify document's update entries.
pub const DOC_UPDATE: u8 = 2;

pub type DocID = u32;

pub fn make_doc_id(name: &[u8]) -> Key<20> {
    let mut v: SmallVec<[u8; 20]> = smallvec![SPACE, DID_SPACE];
    v.write_all(name).unwrap();
    v.push(TERMINATOR);
    Key(v)
}

pub fn doc_name_from_key(key: &[u8]) -> &[u8] {
    &key[2..(key.len() - 1)]
}

pub fn make_doc_state_key(doc_id: DocID) -> Key<8> {
    let mut v: SmallVec<[u8; 8]> = smallvec![SPACE, DOC_SPACE];
    v.write_all(&doc_id.to_be_bytes()).unwrap();
    v.push(DOC_STATE);
    Key(v)
}

// document related elements are stored within bounds [0,1,..did,0]..[0,1,..did,255]
pub fn make_doc_start_key(doc_id: DocID) -> Key<8> {
    make_doc_state_key(doc_id)
}

pub fn make_doc_end_key(doc_id: DocID) -> Key<8> {
    let mut v: SmallVec<[u8; 8]> = smallvec![SPACE, DOC_SPACE];
    v.write_all(&doc_id.to_be_bytes()).unwrap();
    v.push(TERMINATOR_HI_WATERMARK);
    Key(v)
}

pub fn make_state_vector_key(doc_id: DocID) -> Key<8> {
    let mut v: SmallVec<[u8; 8]> = smallvec![SPACE, DOC_SPACE];
    v.write_all(&doc_id.to_be_bytes()).unwrap();
    v.push(DOC_STATE_VEC);
    Key(v)
}

pub fn make_update_key(doc_id: DocID, clock: u32) -> Key<12> {
    let mut v: SmallVec<[u8; 12]> = smallvec![SPACE, DOC_SPACE];
    v.write_all(&doc_id.to_be_bytes()).unwrap();
    v.push(DOC_UPDATE);
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
