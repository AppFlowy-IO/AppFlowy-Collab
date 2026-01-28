use anyhow::anyhow;
use markdown::message::Message;
#[cfg(feature = "lock_timeout")]
use std::time::Duration;
use yrs::TransactionAcqError;

use crate::entity::uuid_validation::RowId;

#[derive(Debug, thiserror::Error)]
pub enum CollabError {
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error("Unexpected empty: {0}")]
  UnexpectedEmpty(String),

  #[error("Get write txn failed")]
  AcquiredWriteTxnFail,

  #[error("Get read txn failed")]
  AcquiredReadTxnFail,

  #[error("Try apply update failed: {0}")]
  YrsTransactionError(String),

  #[error("Try encode update failed: {0}")]
  YrsEncodeStateError(String),

  #[error("UndoManager is not enabled")]
  UndoManagerNotEnabled,

  #[error(transparent)]
  DecodeUpdate(#[from] yrs::encoding::read::Error),

  #[cfg(feature = "plugins")]
  #[error("Rocksdb corruption:{0}")]
  PersistenceRocksdbCorruption(String),

  #[cfg(feature = "plugins")]
  #[error("Rocksdb repair:{0}")]
  PersistenceRocksdbRepairFail(String),

  #[cfg(feature = "plugins")]
  #[error("{0}")]
  PersistenceRocksdbBusy(String),

  #[cfg(feature = "plugins")]
  #[error("{0}")]
  PersistenceRocksdbIOError(String),

  #[error(transparent)]
  Bincode(#[from] bincode::Error),

  #[error("Persistence record not found: {0}")]
  PersistenceRecordNotFound(String),

  #[error("The document already exist")]
  PersistenceDocumentAlreadyExist,

  #[error("Unexpected empty updates")]
  PersistenceUnexpectedEmptyUpdates,

  #[error("invalid data: {0}")]
  PersistenceInvalidData(String),

  #[error("Duplicate update key")]
  PersistenceDuplicateUpdateKey,

  #[error("Can't find the latest update key")]
  PersistenceLatestUpdateKeyNotExist,

  #[error("{0}")]
  NoRequiredData(String),

  #[error("Lack of folder required data:{0}")]
  FolderMissingRequiredData(String),

  #[error(transparent)]
  Awareness(#[from] crate::core::awareness::Error),

  #[error("Failed to apply update: {0}")]
  UpdateFailed(#[from] yrs::error::UpdateError),

  #[error("Document: Could not create block")]
  DocumentBlockCreate,

  #[error("Document: The block already exists")]
  DocumentBlockAlreadyExists,

  #[error("Document: The block is not found")]
  DocumentBlockNotFound,

  #[error("Document: The page id is empty")]
  DocumentPageIdEmpty,

  #[error("Document: Could not convert json to data")]
  DocumentConvertData,

  #[error("Document: The parent is not found")]
  DocumentParentNotFound,

  #[error("Document: Could not create the root block due to an unspecified error")]
  DocumentCreateRootBlock,

  #[error("Document: Could not delete block")]
  DocumentDeleteBlock,

  #[error("Document: text_id or delta is empty")]
  DocumentTextActionParams,

  #[error("Document: Lack of required data")]
  DocumentMissingRequiredData,

  #[error("Document: The external id is not found")]
  DocumentExternalIdNotFound,

  #[error("Document: Unable to parse document to plain text")]
  DocumentParse,

  #[error("Document: Unable to parse markdown to document data")]
  DocumentParseMarkdown,

  #[error("Document: Unable to parse delta json to text delta")]
  DocumentParseDeltaJson,

  #[error("Document: No children found")]
  DocumentNoBlockChildren,

  #[error("Document: Unknown block type: {0}")]
  DocumentUnknownBlockType(String),

  #[error("Document: Unable to find the page block")]
  DocumentPageBlockNotFound,

  #[error("Invalid path: {0}")]
  ImporterInvalidPath(String),

  #[error("Invalid path format")]
  ImporterInvalidPathFormat,

  #[error("{0}")]
  ImporterInvalidFileType(String),

  #[error("Parse markdown error: {0}")]
  ImporterParseMarkdown(Message),

  #[error("File not found")]
  ImporterFileNotFound,

  #[error("Can not import file")]
  ImporterCannotImport,

  #[error("Database: The id is invalid: {0}")]
  DatabaseInvalidId(String),

  #[error("Database: The view id is invalid: {0}")]
  DatabaseInvalidViewId(String),

  #[error("Database: The row id is invalid: {0}")]
  DatabaseInvalidRowId(String),

  #[error("Database: The database is not existing")]
  DatabaseNotExist,

  #[error("Database: row {row_id} not found, reason: {reason}")]
  DatabaseRowNotFound { row_id: RowId, reason: String },

  #[error("Database: The view is not existing")]
  DatabaseViewNotExist,

  #[error("Database: Record already exist")]
  DatabaseRecordAlreadyExist,

  #[error("Database: Record not found")]
  DatabaseRecordNotFound,

  #[error("Database: Action cancelled")]
  DatabaseActionCancelled,

  #[error("Database: Invalid CSV:{0}")]
  DatabaseInvalidCsv(String),

  #[error("Database: Import data failed: {0}")]
  DatabaseImportData(String),

  #[error("Collab version could not be determined")]
  InvalidVersion,

  #[error(transparent)]
  Uuid(#[from] uuid::Error),

  #[error(transparent)]
  Utf8(#[from] std::str::Utf8Error),

  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error("Conversion: invalid structure, expected an object")]
  DeltaNotObject,

  #[error("Conversion: missing 'insert' field")]
  DeltaMissingInsert,

  #[error("Conversion: 'insert' field is not a string")]
  DeltaInsertNotString,

  #[error("Conversion: 'attributes' field is not an object")]
  DeltaAttributesNotObject,

  #[error("Conversion: invalid attribute")]
  DeltaInvalidAttribute,

  #[error("Conversion: invalid insert")]
  DeltaInvalidInsert,

  #[error("cannot fill {0:?} with: {1}")]
  FillInvalidData(yrs::types::TypeRef, String),

  #[cfg(feature = "lock_timeout")]
  #[error("Read lock timeout: {0:?}")]
  RwLockReadTimeout(Duration),

  #[cfg(feature = "lock_timeout")]
  #[error("Write lock timeout: {0:?}")]
  RwLockWriteTimeout(Duration),

  #[cfg(feature = "lock_timeout")]
  #[error("Lock timeout: {0:?}")]
  MutexLockTimeout(Duration),

  #[error("Internal failure: {0}")]
  Internal(#[from] anyhow::Error),
}

impl From<TransactionAcqError> for CollabError {
  fn from(value: TransactionAcqError) -> Self {
    match value {
      TransactionAcqError::SharedAcqFailed => Self::AcquiredReadTxnFail,
      TransactionAcqError::ExclusiveAcqFailed => Self::AcquiredWriteTxnFail,
      TransactionAcqError::DocumentDropped => Self::Internal(anyhow!("Document dropped")),
    }
  }
}

#[cfg(feature = "plugins")]
impl From<rocksdb::Error> for CollabError {
  fn from(value: rocksdb::Error) -> Self {
    match value.kind() {
      rocksdb::ErrorKind::NotFound => Self::PersistenceUnexpectedEmptyUpdates,
      rocksdb::ErrorKind::Corruption => Self::PersistenceRocksdbCorruption(value.into_string()),
      rocksdb::ErrorKind::IOError => Self::PersistenceRocksdbIOError(value.into_string()),
      rocksdb::ErrorKind::Busy => Self::PersistenceRocksdbBusy(value.into_string()),
      _ => Self::Internal(value.into()),
    }
  }
}
