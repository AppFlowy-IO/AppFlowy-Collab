use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  /// Optional external id and type for blocks that are not stored in the document
  pub external_id: Option<String>,
  /// Optional external type for blocks that are not stored in the document
  pub external_type: Option<String>,
  pub data: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentMeta {
  /// Meta has a children map.
  pub children_map: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentData {
  /// Document root block id.
  pub page_id: String,
  /// Document blocks.
  pub blocks: HashMap<String, Block>,
  /// Document meta.
  pub meta: DocumentMeta,
}

/// Operate block action.
#[derive(Debug, Clone, Serialize)]
pub struct BlockAction {
  /// Block action type.
  pub action: BlockActionType,
  /// Block action payload.
  pub payload: BlockActionPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockActionPayload {
  /// Block
  pub block: Block,
  /// Previous block id.
  pub prev_id: Option<String>,
  /// Parent block id.
  pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum BlockActionType {
  Insert,
  Update,
  Delete,
  Move,
}

/// Block change event.
#[derive(Debug, Clone, Serialize)]
pub struct BlockEvent(Vec<BlockEventPayload>);

impl BlockEvent {
  pub fn new(event: Vec<BlockEventPayload>) -> Self {
    Self(event)
  }
}

impl Deref for BlockEvent {
  type Target = Vec<BlockEventPayload>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// Block change event payload.
#[derive(Debug, Clone, Serialize)]
pub struct BlockEventPayload {
  /// change value
  pub value: String,
  /// block map key or children map key
  pub id: String,
  /// eg: ["blocks"] | ["meta", "children_map"]
  pub path: Vec<String>,
  /// delta type
  pub command: DeltaType,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq, Hash)]
pub enum DeltaType {
  Inserted,
  Updated,
  Removed,
}
