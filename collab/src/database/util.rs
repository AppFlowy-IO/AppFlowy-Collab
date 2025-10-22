use crate::database::error::DatabaseError;
use crate::entity::CollabType;
use crate::entity::EncodedCollab;
use crate::preclude::Collab;
pub(crate) fn encoded_collab(
  collab: &Collab,
  collab_type: &CollabType,
) -> Result<EncodedCollab, DatabaseError> {
  let encoded_collab =
    collab.encode_collab_v1(|collab| collab_type.validate_require_data(collab))?;
  Ok(encoded_collab)
}
