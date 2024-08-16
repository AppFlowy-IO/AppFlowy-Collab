use crate::error::DatabaseError;
use anyhow::anyhow;
use collab::entity::EncodedCollab;
use collab::preclude::Collab;
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::CollabKVDB;
use std::sync::Arc;

pub(crate) fn encoded_collab(
  collab: &Collab,
  collab_type: &CollabType,
) -> Result<EncodedCollab, DatabaseError> {
  let encoded_collab = collab
    .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
    .map_err(DatabaseError::Internal)?;

  Ok(encoded_collab)
}

pub(crate) fn write_collab_to_disk(
  uid: i64,
  collab: &Collab,
  collab_type: &CollabType,
  collab_db: &Arc<CollabKVDB>,
) -> Result<(), DatabaseError> {
  let encoded_collab = collab
    .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
    .map_err(DatabaseError::Internal)?;

  collab_db
    .with_write_txn(|txn| {
      txn.flush_doc(
        uid,
        collab.object_id(),
        encoded_collab.state_vector.to_vec(),
        encoded_collab.doc_state.to_vec(),
      )?;
      Ok(())
    })
    .map_err(|err| {
      DatabaseError::Internal(anyhow!("flush doc:{} failed: {}", collab.object_id(), err))
    })
}
