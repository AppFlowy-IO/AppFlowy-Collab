use crate::database::database::timestamp;
use crate::preclude::{Any, Map, MapExt, MapRef, ReadTxn, TransactionMut};
use crate::util::deserialize_i64_from_numeric;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Key constants for comment fields in MapRef
pub const COMMENT_ID: &str = "id";
pub const COMMENT_PARENT_ID: &str = "parent_comment_id";
pub const COMMENT_CONTENT: &str = "content";
pub const COMMENT_AUTHOR_ID: &str = "author_id";
pub const COMMENT_CREATED_AT: &str = "created_at";
pub const COMMENT_UPDATED_AT: &str = "updated_at";
pub const COMMENT_IS_RESOLVED: &str = "is_resolved";
pub const COMMENT_RESOLVED_BY: &str = "resolved_by";
pub const COMMENT_RESOLVED_AT: &str = "resolved_at";
pub const COMMENT_REACTIONS: &str = "reactions";
pub const COMMENT_ATTACHMENTS: &str = "attachments";

/// Represents an attachment on a comment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommentAttachment {
  /// Unique identifier for the attachment (UUID)
  pub id: String,
  /// Original file name
  pub name: String,
  /// URL of the uploaded file
  pub url: String,
  /// MIME type or file type (e.g., "image/png", "application/pdf")
  pub file_type: String,
  /// File size in bytes
  pub size: i64,
  /// Timestamp when attachment was uploaded
  pub uploaded_at: i64,
}

impl CommentAttachment {
  /// Creates a new attachment
  pub fn new(name: String, url: String, file_type: String, size: i64) -> Self {
    Self {
      id: Uuid::new_v4().to_string(),
      name,
      url,
      file_type,
      size,
      uploaded_at: timestamp(),
    }
  }
}

/// Represents a comment on a database row.
/// Comments are stored in a MapRef keyed by comment_id for O(1) lookup.
/// Threading is implemented via flat structure with parent_comment_id field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RowComment {
  /// Unique identifier for the comment (UUID)
  pub id: String,
  /// Parent comment ID for replies (None for top-level comments)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_comment_id: Option<String>,
  /// Rich text content as JSON string
  pub content: String,
  /// User ID of the comment author
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  pub author_id: i64,
  /// Timestamp when comment was created
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  pub created_at: i64,
  /// Timestamp when comment was last updated
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  pub updated_at: i64,
  /// Whether the comment thread is resolved
  #[serde(default)]
  pub is_resolved: bool,
  /// User ID who resolved the comment
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resolved_by: Option<String>,
  /// Timestamp when comment was resolved
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resolved_at: Option<i64>,
  /// Reactions on the comment: emoji -> list of user IDs
  #[serde(default)]
  pub reactions: HashMap<String, Vec<i64>>,
  /// Attachments on the comment
  #[serde(default)]
  pub attachments: Vec<CommentAttachment>,
}

impl RowComment {
  /// Creates a new top-level comment
  pub fn new(content: String, author_id: i64) -> Self {
    let now = timestamp();
    Self {
      id: Uuid::new_v4().to_string(),
      parent_comment_id: None,
      content,
      author_id,
      created_at: now,
      updated_at: now,
      is_resolved: false,
      resolved_by: None,
      resolved_at: None,
      reactions: HashMap::new(),
      attachments: Vec::new(),
    }
  }

  /// Creates a new top-level comment with attachments
  pub fn new_with_attachments(
    content: String,
    author_id: i64,
    attachments: Vec<CommentAttachment>,
  ) -> Self {
    let now = timestamp();
    Self {
      id: Uuid::new_v4().to_string(),
      parent_comment_id: None,
      content,
      author_id,
      created_at: now,
      updated_at: now,
      is_resolved: false,
      resolved_by: None,
      resolved_at: None,
      reactions: HashMap::new(),
      attachments,
    }
  }

  /// Creates a new reply comment
  pub fn new_reply(content: String, author_id: i64, parent_comment_id: String) -> Self {
    let now = timestamp();
    Self {
      id: Uuid::new_v4().to_string(),
      parent_comment_id: Some(parent_comment_id),
      content,
      author_id,
      created_at: now,
      updated_at: now,
      is_resolved: false,
      resolved_by: None,
      resolved_at: None,
      reactions: HashMap::new(),
      attachments: Vec::new(),
    }
  }

  /// Creates a RowComment from a MapRef
  pub fn from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Self> {
    let id: String = map_ref.get_with_txn(txn, COMMENT_ID)?;
    let parent_comment_id: Option<String> = map_ref.get_with_txn(txn, COMMENT_PARENT_ID);
    let content: String = map_ref
      .get_with_txn(txn, COMMENT_CONTENT)
      .unwrap_or_default();
    let author_id: i64 = map_ref.get_with_txn(txn, COMMENT_AUTHOR_ID).unwrap_or(0);
    let created_at: i64 = map_ref.get_with_txn(txn, COMMENT_CREATED_AT).unwrap_or(0);
    let updated_at: i64 = map_ref.get_with_txn(txn, COMMENT_UPDATED_AT).unwrap_or(0);
    let is_resolved: bool = map_ref
      .get_with_txn(txn, COMMENT_IS_RESOLVED)
      .unwrap_or(false);
    let resolved_by: Option<String> = map_ref.get_with_txn(txn, COMMENT_RESOLVED_BY);
    let resolved_at: Option<i64> = map_ref.get_with_txn(txn, COMMENT_RESOLVED_AT);

    // Parse reactions from JSON string
    let reactions_str: Option<String> = map_ref.get_with_txn(txn, COMMENT_REACTIONS);
    let reactions = reactions_str
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default();

    // Parse attachments from JSON string
    let attachments_str: Option<String> = map_ref.get_with_txn(txn, COMMENT_ATTACHMENTS);
    let attachments = attachments_str
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default();

    Some(Self {
      id,
      parent_comment_id,
      content,
      author_id,
      created_at,
      updated_at,
      is_resolved,
      resolved_by,
      resolved_at,
      reactions,
      attachments,
    })
  }

  /// Fills a MapRef with the comment data
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    map_ref.insert(txn, COMMENT_ID, self.id);
    if let Some(parent_id) = self.parent_comment_id {
      map_ref.insert(txn, COMMENT_PARENT_ID, parent_id);
    }
    map_ref.insert(txn, COMMENT_CONTENT, self.content);
    map_ref.insert(txn, COMMENT_AUTHOR_ID, Any::BigInt(self.author_id));
    map_ref.insert(txn, COMMENT_CREATED_AT, Any::BigInt(self.created_at));
    map_ref.insert(txn, COMMENT_UPDATED_AT, Any::BigInt(self.updated_at));
    map_ref.insert(txn, COMMENT_IS_RESOLVED, self.is_resolved);
    if let Some(resolved_by) = self.resolved_by {
      map_ref.insert(txn, COMMENT_RESOLVED_BY, resolved_by);
    }
    if let Some(resolved_at) = self.resolved_at {
      map_ref.insert(txn, COMMENT_RESOLVED_AT, Any::BigInt(resolved_at));
    }
    // Store reactions as JSON string
    if !self.reactions.is_empty() {
      if let Ok(reactions_json) = serde_json::to_string(&self.reactions) {
        map_ref.insert(txn, COMMENT_REACTIONS, reactions_json);
      }
    }
    // Store attachments as JSON string
    if !self.attachments.is_empty() {
      if let Ok(attachments_json) = serde_json::to_string(&self.attachments) {
        map_ref.insert(txn, COMMENT_ATTACHMENTS, attachments_json);
      }
    }
  }
}

impl TryFrom<Any> for RowComment {
  type Error = anyhow::Error;

  fn try_from(value: Any) -> Result<Self, Self::Error> {
    let mut json = String::new();
    value.to_json(&mut json);
    let comment = serde_json::from_str(&json)?;
    Ok(comment)
  }
}

impl From<RowComment> for Any {
  fn from(item: RowComment) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    Any::from_json(&json).unwrap()
  }
}

/// Changeset for comment operations
#[derive(Debug, Clone)]
pub enum RowCommentChange {
  /// A new comment was added
  DidAddComment { comment: RowComment },
  /// A comment was updated
  DidUpdateComment { comment: RowComment },
  /// A comment was deleted
  DidDeleteComment { comment_id: String },
}
