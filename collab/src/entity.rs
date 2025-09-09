use crate::error::CollabError;
use bytes::Bytes;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::{Debug, Formatter};
use yrs::Update;
use yrs::updates::decoder::Decode;

/// Magic bytes to identify protobuf-encoded EncodedCollab data
/// Using "AFPB" (AppFlowy ProtoBuf) as magic bytes: [0x41, 0x46, 0x50, 0x42]
const PROTOBUF_MAGIC_BYTES: &[u8] = b"AFPB";

#[derive(Clone, Eq, PartialEq)]
pub struct EncodedCollab {
  pub state_vector: Bytes,
  pub doc_state: Bytes,
  pub version: EncoderVersion,
}

/// Internal protobuf representation for encoding/decoding
#[derive(Clone, PartialEq, Message)]
struct ProtoEncodedCollab {
  #[prost(bytes = "vec", tag = "1")]
  state_vector: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  doc_state: Vec<u8>,
  #[prost(int32, tag = "3")]
  encoder_version: i32,
}

impl From<i32> for EncoderVersion {
  fn from(value: i32) -> Self {
    match value {
      0 => EncoderVersion::V1,
      1 => EncoderVersion::V2,
      _ => EncoderVersion::V1,
    }
  }
}

impl From<EncoderVersion> for i32 {
  fn from(version: EncoderVersion) -> Self {
    match version {
      EncoderVersion::V1 => 0,
      EncoderVersion::V2 => 1,
    }
  }
}

/// Helper struct for bincode serialization/deserialization
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct BincodeEncodedCollab {
  pub state_vector: Bytes,
  pub doc_state: Bytes,
  #[serde(default)]
  pub version: EncoderVersion,
}

impl From<EncodedCollab> for BincodeEncodedCollab {
  fn from(value: EncodedCollab) -> Self {
    BincodeEncodedCollab {
      state_vector: value.state_vector,
      doc_state: value.doc_state,
      version: value.version,
    }
  }
}

impl From<BincodeEncodedCollab> for EncodedCollab {
  fn from(value: BincodeEncodedCollab) -> Self {
    EncodedCollab {
      state_vector: value.state_vector,
      doc_state: value.doc_state,
      version: value.version,
    }
  }
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

#[derive(Default, Serialize_repr, Deserialize_repr, Eq, PartialEq, Debug, Clone, Copy)]
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

  pub fn encode_to_bytes(&self) -> Result<Vec<u8>, CollabError> {
    Ok(self.encode_to_protobuf())
  }

  fn encode_to_protobuf(&self) -> Vec<u8> {
    let proto = self.to_proto();
    let proto_bytes = proto.encode_to_vec();

    let mut result = Vec::with_capacity(PROTOBUF_MAGIC_BYTES.len() + proto_bytes.len());
    result.extend_from_slice(PROTOBUF_MAGIC_BYTES);
    result.extend_from_slice(&proto_bytes);
    result
  }

  fn to_proto(&self) -> ProtoEncodedCollab {
    ProtoEncodedCollab {
      state_vector: self.state_vector.to_vec(),
      doc_state: self.doc_state.to_vec(),
      encoder_version: self.version as i32,
    }
  }

  fn from_proto(proto: ProtoEncodedCollab) -> Self {
    EncodedCollab {
      state_vector: Bytes::from(proto.state_vector),
      doc_state: Bytes::from(proto.doc_state),
      version: proto.encoder_version.into(),
    }
  }

  pub fn decode_from_bytes(encoded: &[u8]) -> Result<EncodedCollab, CollabError> {
    // Fast check: if it starts with magic bytes, it's protobuf format
    if encoded.len() >= PROTOBUF_MAGIC_BYTES.len() && encoded.starts_with(PROTOBUF_MAGIC_BYTES) {
      // Remove magic bytes and decode protobuf
      let proto_data = &encoded[PROTOBUF_MAGIC_BYTES.len()..];
      if let Ok(proto_collab) = ProtoEncodedCollab::decode(proto_data) {
        return Ok(Self::from_proto(proto_collab));
      }
    }

    // Try current bincode format
    if let Ok(bincode_collab) = bincode::deserialize::<BincodeEncodedCollab>(encoded) {
      return Ok(EncodedCollab {
        state_vector: bincode_collab.state_vector,
        doc_state: bincode_collab.doc_state,
        version: bincode_collab.version,
      });
    }

    // Try legacy bincode format (EncodedCollabV0)
    if let Ok(old_collab) = bincode::deserialize::<EncodedCollabV0>(encoded) {
      return Ok(EncodedCollab {
        state_vector: old_collab.state_vector,
        doc_state: old_collab.doc_state,
        version: EncoderVersion::V1,
      });
    }

    Err(CollabError::NoDecodingFormat)
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
    let old_encoded_collab = EncodedCollab::decode_from_bytes(&new_encoded_collab_bytes).unwrap();

    assert_eq!(old_encoded_collab.doc_state, new_encoded_collab.doc_state);
    assert_eq!(
      old_encoded_collab.state_vector,
      new_encoded_collab.state_vector
    );
  }

  #[test]
  fn test_protobuf_encoding_decoding() {
    let original = EncodedCollab {
      state_vector: Bytes::from(vec![1, 2, 3]),
      doc_state: Bytes::from(vec![4, 5, 6]),
      version: EncoderVersion::V1,
    };

    // Test protobuf encoding and decoding
    let protobuf_bytes = original.encode_to_protobuf();
    let decoded = EncodedCollab::decode_from_bytes(&protobuf_bytes).unwrap();

    assert_eq!(original, decoded);
  }

  #[test]
  fn test_protobuf_encoding_with_v2() {
    let original = EncodedCollab {
      state_vector: Bytes::from(vec![7, 8, 9]),
      doc_state: Bytes::from(vec![10, 11, 12]),
      version: EncoderVersion::V2,
    };

    let protobuf_bytes = original.encode_to_protobuf();
    let decoded = EncodedCollab::decode_from_bytes(&protobuf_bytes).unwrap();

    assert_eq!(original, decoded);
  }

  #[test]
  fn test_backward_compatibility_all_formats() {
    let original = EncodedCollab {
      state_vector: Bytes::from(vec![19, 20, 21]),
      doc_state: Bytes::from(vec![22, 23, 24]),
      version: EncoderVersion::V1,
    };

    // Test that legacy bincode format can still be decoded
    let bincode_collab = BincodeEncodedCollab::from(original.clone());
    let legacy_bytes = bincode::serialize(&bincode_collab).unwrap();
    let decoded_legacy = EncodedCollab::decode_from_bytes(&legacy_bytes).unwrap();
    assert_eq!(original, decoded_legacy);

    // Test that protobuf format can be decoded
    let protobuf_bytes = original.encode_to_protobuf();
    let decoded_protobuf = EncodedCollab::decode_from_bytes(&protobuf_bytes).unwrap();
    assert_eq!(original, decoded_protobuf);

    // Test legacy v0 format still works
    let legacy_v0 = EncodedCollabV0 {
      state_vector: original.state_vector.clone(),
      doc_state: original.doc_state.clone(),
    };
    let legacy_v0_bytes = bincode::serialize(&legacy_v0).unwrap();
    let decoded_v0 = EncodedCollab::decode_from_bytes(&legacy_v0_bytes).unwrap();

    let expected_from_v0 = EncodedCollab {
      state_vector: original.state_vector,
      doc_state: original.doc_state,
      version: EncoderVersion::V1, // Should default to V1 when converting from V0
    };
    assert_eq!(expected_from_v0, decoded_v0);
  }

  #[test]
  fn test_invalid_data_handling() {
    let invalid_data = vec![255, 254, 253, 252];
    let result = EncodedCollab::decode_from_bytes(&invalid_data);

    assert!(result.is_err());
    match result.unwrap_err() {
      CollabError::NoDecodingFormat => {}, // Expected
      _ => panic!("Expected NoDecodingFormat error"),
    }
  }
}
