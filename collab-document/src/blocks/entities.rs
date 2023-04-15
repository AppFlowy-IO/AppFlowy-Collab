use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone, Serialize)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub external_id: Option<String>,
  pub external_type: Option<String>,
  pub data: HashMap<String, Value>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DocumentMeta {
  pub children_map: HashMap<String, Vec<String>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DocumentData {
  pub page_id: String,
  pub blocks: HashMap<String, Block>,
  pub meta: DocumentMeta,
}
#[derive(Debug, Clone, Serialize)]
pub struct BlockAction {
  pub action: BlockActionType,
  pub payload: BlockActionPayload,
}
#[derive(Debug, Clone, Serialize)]
pub struct BlockActionPayload {
  pub block: Block,
  pub prev_id: Option<String>,
  pub parent_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub enum BlockActionType {
  Insert,
  Update,
  Delete,
  Move,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct BlockEventPayload {
  pub value: String,
  pub id: String,
  pub path: Vec<String>,
  pub command: DeltaType,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq, Hash)]
pub enum DeltaType {
  Inserted,
  Updated,
  Removed,
}
