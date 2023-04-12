use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

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
pub struct BlockEvent {
  pub path: Vec<String>,
  pub delta: Vec<Delta>,
}

#[derive(Debug, Clone, Serialize)]
pub enum Delta {
  Array(ArrayDelta),
  Map(MapDelta),
}

#[derive(Debug, Clone, Serialize)]
pub enum ArrayDelta {
  Added(Vec<String>),
  Removed(u32),
  Retain(u32),
}

#[derive(Debug, Clone, Serialize)]
pub enum MapDelta {
  // id, content
  Inserted(String, Value),
  // path, old value, new value
  Updated(String, Value, Value),
  // id
  Removed(String),
}
