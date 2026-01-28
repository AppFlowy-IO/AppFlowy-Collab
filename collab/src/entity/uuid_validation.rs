use anyhow::{Result, anyhow};
use uuid::Uuid;

// Re-export type aliases from define module
pub use super::define::{
  BlockId, DatabaseId, DatabaseViewId, DocumentId, ObjectId, RowId, ViewId, WorkspaceId,
};

/// Validates that a string is a valid UUID format
pub fn validate_uuid_string(id: &str) -> Result<()> {
  if id.is_empty() {
    return Err(anyhow!("Empty ID provided"));
  }

  Uuid::parse_str(id).map_err(|_| anyhow!("Invalid UUID format: {}", id))?;

  Ok(())
}

/// Validates a database ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_database_id(_id: &DatabaseId) -> Result<()> {
  Ok(())
}

/// Validates a database view ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_database_view_id(_id: &DatabaseViewId) -> Result<()> {
  Ok(())
}

/// Validates a document ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_document_id(_id: &DocumentId) -> Result<()> {
  Ok(())
}

/// Validates an object ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_object_id(_id: &ObjectId) -> Result<()> {
  Ok(())
}

/// Validates a view ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_view_id(_id: &ViewId) -> Result<()> {
  Ok(())
}

/// Validates a workspace ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_workspace_id(_id: &WorkspaceId) -> Result<()> {
  Ok(())
}

/// Generates a new valid UUID for use as database ID
pub fn generate_database_id() -> DatabaseId {
  Uuid::new_v4()
}

/// Generates a new valid UUID for use as database view ID
pub fn generate_database_view_id() -> DatabaseViewId {
  Uuid::new_v4()
}

/// Generates a new valid UUID for use as document ID
pub fn generate_document_id() -> DocumentId {
  Uuid::new_v4()
}

/// Generates a new valid UUID for use as object ID
pub fn generate_object_id() -> ObjectId {
  Uuid::new_v4()
}

pub fn generate_view_id() -> ViewId {
  Uuid::new_v4()
}

pub fn generate_workspace_id() -> WorkspaceId {
  Uuid::new_v4()
}

/// Generates a new valid UUID for use as row ID
pub fn generate_row_id() -> RowId {
  Uuid::new_v4()
}

/// Validate and convert string to UUID if valid, otherwise return error
pub fn ensure_uuid(id: &str) -> Result<Uuid> {
  validate_uuid_string(id)?;
  Ok(Uuid::parse_str(id).unwrap()) // Safe to unwrap after validation
}

/// Batch validate multiple IDs
pub fn validate_batch_ids(ids: &[String]) -> Result<()> {
  for (index, id) in ids.iter().enumerate() {
    validate_uuid_string(id).map_err(|e| anyhow!("Invalid UUID at index {}: {}", index, e))?;
  }
  Ok(())
}

/// Helper for validating IDs in constructors with context
pub fn validate_with_context(id: &str, _context: &str) -> bool {
  validate_uuid_string(id).is_ok()
}

/// Try to parse a string as a DatabaseId, returning None if it's not a valid UUID
pub fn try_parse_database_id(id: &str) -> Option<DatabaseId> {
  Uuid::parse_str(id).ok()
}

/// Try to parse a string as a DatabaseViewId, returning None if it's not a valid UUID
pub fn try_parse_database_view_id(id: &str) -> Option<DatabaseViewId> {
  Uuid::parse_str(id).ok()
}

/// Convert any string to DocumentId using deterministic UUID generation for testing
/// This is used in tests where predictable string IDs are needed
pub fn document_id_from_any_string(id: &str) -> DocumentId {
  if let Ok(uuid) = Uuid::parse_str(id) {
    uuid
  } else {
    // For non-UUID strings, create a deterministic UUID based on the string
    Uuid::new_v5(&Uuid::NAMESPACE_OID, id.as_bytes())
  }
}

/// Convert any string to ObjectId using deterministic UUID generation for testing
/// This is used in tests where predictable string IDs are needed
pub fn object_id_from_any_string(id: &str) -> ObjectId {
  if let Ok(uuid) = Uuid::parse_str(id) {
    uuid
  } else {
    // For non-UUID strings, create a deterministic UUID based on the string
    Uuid::new_v5(&Uuid::NAMESPACE_OID, id.as_bytes())
  }
}

/// Convert any string to ViewId using deterministic UUID generation for testing
/// This is used in tests where predictable string IDs are needed
pub fn view_id_from_any_string(id: &str) -> ViewId {
  if let Ok(uuid) = Uuid::parse_str(id) {
    uuid
  } else {
    // For non-UUID strings, create a deterministic UUID based on the string
    Uuid::new_v5(&Uuid::NAMESPACE_OID, id.as_bytes())
  }
}

/// Validates a row ID is a valid UUID (always succeeds since type is Uuid)
pub fn validate_row_id(_id: &RowId) -> Result<()> {
  Ok(())
}

/// Convert string to RowId, validating first
pub fn row_id_from_string(id: &str) -> Result<RowId> {
  validate_uuid_string(id)?;
  Ok(Uuid::parse_str(id).unwrap())
}

// Compatibility aliases for common operations
pub use self::generate_database_id as gen_database_id;
pub use self::generate_database_view_id as gen_database_view_id;
pub use self::generate_document_id as gen_document_id;
pub use self::generate_object_id as gen_object_id;
pub use self::generate_row_id as gen_row_id;
pub use self::generate_view_id as gen_view_id;
pub use self::generate_workspace_id as gen_workspace_id;

/// Convert any string to BlockId using deterministic UUID generation for testing
/// This is used in tests where predictable string IDs are needed
pub fn block_id_from_any_string(id: &str) -> BlockId {
  if let Ok(uuid) = Uuid::parse_str(id) {
    uuid
  } else {
    // For non-UUID strings, create a deterministic UUID based on the string
    Uuid::new_v5(&Uuid::NAMESPACE_OID, id.as_bytes())
  }
}
