use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub external_id: Option<String>,
  pub external_type: Option<String>,
  pub data: HashMap<String, Value>,
}

pub struct DocumentMeta {
  pub children_map: HashMap<String, Vec<String>>,
}

pub struct DocumentData {
  pub page_id: String,
  pub blocks: HashMap<String, Block>,
  pub meta: DocumentMeta,
}

pub struct BlockAction {
  pub action: BlockActionType,
  pub payload: BlockActionPayload,
}

pub struct BlockActionPayload {
  pub block: Block,
  pub prev_id: Option<String>,
  pub parent_id: Option<String>,
}
pub enum BlockActionType {
  Insert,
  Update,
  Delete,
  Move,
}

#[derive(Debug, Clone)]
pub struct BlockEvent {
  pub path: Vec<String>,
  pub delta: Vec<Delta>,
}

#[derive(Debug, Clone)]
pub enum Delta {
  Array(ArrayDelta),
  Map(MapDelta),
}

#[derive(Debug, Clone)]
pub enum ArrayDelta {
  Added(Vec<String>),
  Removed(u32),
  Retain(u32),
}

#[derive(Debug, Clone)]
pub enum MapDelta {
  // id, content
  Inserted(String, Value),
  // path, old value, new value
  Updated(String, Value, Value),
  // id
  Removed(String),
}
