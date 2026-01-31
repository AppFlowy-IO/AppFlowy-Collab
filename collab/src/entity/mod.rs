pub mod collab_object;
pub mod define;
pub mod proto;
pub mod reminder;
pub mod uuid_validation;

pub use collab_object::*;

use crate::core::collab::{CollabVersion, VersionedData};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use yrs::Update;
use yrs::updates::decoder::Decode;

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Default, Hash, Debug)]
#[serde(transparent)]
pub struct CollabStateVector(pub Bytes);

impl CollabStateVector {
  pub fn as_bytes(&self) -> &Bytes {
    &self.0
  }

  pub fn into_bytes(self) -> Bytes {
    self.0
  }
}

impl Deref for CollabStateVector {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.0.as_ref()
  }
}

impl AsRef<[u8]> for CollabStateVector {
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl From<Bytes> for CollabStateVector {
  fn from(value: Bytes) -> Self {
    Self(value)
  }
}

impl From<Vec<u8>> for CollabStateVector {
  fn from(value: Vec<u8>) -> Self {
    Self(Bytes::from(value))
  }
}

impl From<CollabStateVector> for Bytes {
  fn from(value: CollabStateVector) -> Self {
    value.0
  }
}

impl From<&CollabStateVector> for Bytes {
  fn from(value: &CollabStateVector) -> Self {
    value.0.clone()
  }
}

impl From<CollabStateVector> for Vec<u8> {
  fn from(value: CollabStateVector) -> Self {
    value.0.to_vec()
  }
}

impl From<&CollabStateVector> for Vec<u8> {
  fn from(value: &CollabStateVector) -> Self {
    value.0.to_vec()
  }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Default, Hash, Debug)]
#[serde(transparent)]
pub struct CollabDocState(pub Bytes);

impl CollabDocState {
  pub fn as_bytes(&self) -> &Bytes {
    &self.0
  }

  pub fn into_bytes(self) -> Bytes {
    self.0
  }
}

impl Deref for CollabDocState {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.0.as_ref()
  }
}

impl AsRef<[u8]> for CollabDocState {
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl From<Bytes> for CollabDocState {
  fn from(value: Bytes) -> Self {
    Self(value)
  }
}

impl From<Vec<u8>> for CollabDocState {
  fn from(value: Vec<u8>) -> Self {
    Self(Bytes::from(value))
  }
}

impl From<CollabDocState> for Bytes {
  fn from(value: CollabDocState) -> Self {
    value.0
  }
}

impl From<&CollabDocState> for Bytes {
  fn from(value: &CollabDocState) -> Self {
    value.0.clone()
  }
}

impl From<CollabDocState> for Vec<u8> {
  fn from(value: CollabDocState) -> Self {
    value.0.to_vec()
  }
}

impl From<&CollabDocState> for Vec<u8> {
  fn from(value: &CollabDocState) -> Self {
    value.0.to_vec()
  }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct EncodedCollab {
  pub state_vector: CollabStateVector,
  pub doc_state: CollabDocState,
  #[serde(default)]
  pub version: EncoderVersion,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub collab_version: Option<CollabVersion>,
}

impl Debug for EncodedCollab {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let doc_state = match self.version {
      EncoderVersion::V1 => Update::decode_v1(self.doc_state.as_ref()),
      EncoderVersion::V2 => Update::decode_v2(self.doc_state.as_ref()),
    }
    .map_err(|_| std::fmt::Error)?;

    f.debug_struct("EncodedCollab")
      .field("state_vector", &self.state_vector)
      .field("doc_state", &doc_state)
      .field("version", &self.version)
      .finish()
  }
}

#[derive(Default, Serialize_repr, Deserialize_repr, Eq, PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum EncoderVersion {
  #[default]
  V1 = 0,
  V2 = 1,
}

impl EncodedCollab {
  pub fn new_v1<S: Into<CollabStateVector>, D: Into<CollabDocState>>(
    state_vector: S,
    doc_state: D,
  ) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V1,
      collab_version: None,
    }
  }

  pub fn new_v2<S: Into<CollabStateVector>, D: Into<CollabDocState>>(
    state_vector: S,
    doc_state: D,
  ) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V2,
      collab_version: None,
    }
  }

  pub fn versioned_data(self) -> VersionedData {
    VersionedData::new(self.doc_state, self.collab_version)
  }

  pub fn encode_to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(self)
  }

  pub fn decode_from_bytes(encoded: &[u8]) -> Result<EncodedCollab, bincode::Error> {
    // The deserialize_encoded_collab function first tries to deserialize the data as EncodedCollab.
    // If it fails (presumably because the data was serialized with EncodedCollabV0), it then tries to deserialize as EncodedCollabV0.
    // After successfully deserializing as EncodedCollabV0, it constructs a new EncodedCollab object with the data from
    // EncodedCollabV0 and sets the version to a default value.
    match bincode::deserialize::<EncodedCollab>(encoded) {
      Ok(new_collab) => Ok(new_collab),
      Err(_) => {
        let old_collab: EncodedCollabV0 = bincode::deserialize(encoded)?;
        Ok(EncodedCollab {
          state_vector: CollabStateVector::from(old_collab.state_vector),
          doc_state: CollabDocState::from(old_collab.doc_state),
          version: EncoderVersion::V1,
          collab_version: None,
        })
      },
    }
  }
}

#[derive(Serialize, Deserialize)]
pub struct EncodedCollabV0 {
  pub state_vector: Bytes,
  pub doc_state: Bytes,
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn old_encoded_collab_decoded_into_new_encoded_collab() {
    let old_encoded_collab = EncodedCollabV0 {
      state_vector: Bytes::from(vec![1, 2, 3]),
      doc_state: Bytes::from(vec![4, 5, 6]),
    };

    let old_encoded_collab_bytes = bincode::serialize(&old_encoded_collab).unwrap();
    let new_encoded_collab = EncodedCollab::decode_from_bytes(&old_encoded_collab_bytes).unwrap();

    assert_eq!(
      new_encoded_collab,
      EncodedCollab {
        state_vector: CollabStateVector::from(vec![1, 2, 3]),
        doc_state: CollabDocState::from(vec![4, 5, 6]),
        version: EncoderVersion::V1,
        collab_version: None,
      }
    );
  }

  #[test]
  fn new_encoded_collab_decoded_into_old_encoded_collab() {
    let new_encoded_collab = EncodedCollab {
      state_vector: CollabStateVector::from(vec![1, 2, 3]),
      doc_state: CollabDocState::from(vec![4, 5, 6]),
      version: EncoderVersion::V1,
      collab_version: None,
    };

    let new_encoded_collab_bytes = new_encoded_collab.encode_to_bytes().unwrap();
    let old_encoded_collab: EncodedCollabV0 =
      bincode::deserialize(&new_encoded_collab_bytes).unwrap();

    assert_eq!(
      old_encoded_collab.doc_state,
      Bytes::from(&new_encoded_collab.doc_state)
    );
    assert_eq!(
      old_encoded_collab.state_vector,
      Bytes::from(&new_encoded_collab.state_vector)
    );
  }
}
