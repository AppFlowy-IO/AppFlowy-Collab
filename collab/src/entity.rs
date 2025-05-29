use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::{Debug, Formatter};
use yrs::Update;
use yrs::updates::decoder::Decode;

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct EncodedCollab {
  pub state_vector: Bytes,
  pub doc_state: Bytes,
  #[serde(default)]
  pub version: EncoderVersion,
}

impl Debug for EncodedCollab {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let doc_state = match self.version {
      EncoderVersion::V1 => Update::decode_v1(&self.doc_state),
      EncoderVersion::V2 => Update::decode_v2(&self.doc_state),
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
  pub fn new_v1<T: Into<Bytes>>(state_vector: T, doc_state: T) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V1,
    }
  }

  pub fn new_v2<T: Into<Bytes>>(state_vector: T, doc_state: T) -> Self {
    Self {
      state_vector: state_vector.into(),
      doc_state: doc_state.into(),
      version: EncoderVersion::V2,
    }
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
          state_vector: old_collab.state_vector,
          doc_state: old_collab.doc_state,
          version: EncoderVersion::V1,
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
        state_vector: Bytes::from(vec![1, 2, 3]),
        doc_state: Bytes::from(vec![4, 5, 6]),
        version: EncoderVersion::V1,
      }
    );
  }

  #[test]
  fn new_encoded_collab_decoded_into_old_encoded_collab() {
    let new_encoded_collab = EncodedCollab {
      state_vector: Bytes::from(vec![1, 2, 3]),
      doc_state: Bytes::from(vec![4, 5, 6]),
      version: EncoderVersion::V1,
    };

    let new_encoded_collab_bytes = new_encoded_collab.encode_to_bytes().unwrap();
    let old_encoded_collab: EncodedCollabV0 =
      bincode::deserialize(&new_encoded_collab_bytes).unwrap();

    assert_eq!(old_encoded_collab.doc_state, new_encoded_collab.doc_state);
    assert_eq!(
      old_encoded_collab.state_vector,
      new_encoded_collab.state_vector
    );
  }
}
