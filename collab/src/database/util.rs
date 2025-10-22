use crate::entity::CollabType;
use crate::entity::EncodedCollab;
use crate::error::CollabError;
use crate::preclude::Collab;
pub(crate) fn encoded_collab(
  collab: &Collab,
  collab_type: &CollabType,
) -> Result<EncodedCollab, CollabError> {
  let encoded_collab =
    collab.encode_collab_v1(|collab| collab_type.validate_require_data(collab))?;
  Ok(encoded_collab)
}
