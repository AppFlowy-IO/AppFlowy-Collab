use crate::error::DatabaseError;
use collab::entity::EncodedCollab;
use collab::preclude::Collab;
use collab_entity::CollabType;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
pub(crate) fn encoded_collab(
  collab: &Collab,
  collab_type: &CollabType,
) -> Result<EncodedCollab, DatabaseError> {
  let encoded_collab =
    collab.encode_collab_v1(|collab| collab_type.validate_require_data(collab))?;
  Ok(encoded_collab)
}

pub fn upload_file_url(host: &str, workspace_id: &str, object_id: &str, file_id: &str) -> String {
  let parent_dir = utf8_percent_encode(object_id, NON_ALPHANUMERIC).to_string();
  format!("{host}/{workspace_id}/v1/blob/{parent_dir}/{file_id}",)
}
