use crate::blocks::{BlockDataEnum, BlockType};
use anyhow::Result;
use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

const ID: &str = "id";
const TYPE: &str = "ty";
const CHILDREN: &str = "children";
const PARENT: &str = "parent";
const DATA: &str = "data";

#[derive(Deserialize, Debug)]
pub struct Block {
  pub id: String,

  pub ty: BlockType,

  pub parent: String,

  pub children: String,

  pub data: BlockDataEnum,
}

impl Serialize for Block {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut s = serializer.serialize_struct("Block", 5)?;
    s.serialize_field("id", &self.id)?;
    s.serialize_field("ty", &self.ty)?;
    s.serialize_field("parent", &self.parent)?;
    s.serialize_field("children", &self.children)?;
    s.serialize_field("data", &self.data.to_json_value())?;
    s.end()
  }
}
pub struct BlockMap {
  root: MapRefWrapper,
}

impl Serialize for BlockMap {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let mut map = serializer.serialize_map(Some(self.root.len(&txn) as usize))?;
    for (k, _) in self.root.iter(&txn) {
      // It's safe to unwrap, because we know the key exists.
      let block = self.get_block(&txn, k).unwrap();
      let value = serde_json::to_value(block).unwrap_or_default();
      map.serialize_entry(k, &value)?;
    }
    map.end()
  }
}

impl BlockMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json_value(&self) -> serde_json::Value {
    serde_json::to_value(self).unwrap_or_default()
  }

  pub fn get_block<T: ReadTxn>(&self, txn: &T, block_id: &str) -> Option<Block> {
    let block_map = self.root.get_map_with_txn(txn, block_id);
    block_map.map(|block_map| self.get_block_by_map(txn, block_map))
  }

  pub fn get_block_by_map<T: ReadTxn>(&self, txn: &T, block_map: MapRefWrapper) -> Block {
    let id = block_map.get_str_with_txn(txn, ID).unwrap_or_default();
    let ty = block_map.get_str_with_txn(txn, TYPE).unwrap_or_default();
    let parent = block_map.get_str_with_txn(txn, PARENT).unwrap_or_default();
    let children = block_map
      .get_str_with_txn(txn, CHILDREN)
      .unwrap_or_default();
    let data = block_map.get_str_with_txn(txn, DATA).unwrap_or_default();
    Block {
      id,
      ty: BlockType::from_string(&ty),
      parent,
      children,
      data: BlockDataEnum::from_string(&data),
    }
  }

  pub fn create_block(
    &self,
    txn: &mut TransactionMut,
    block_id: String,
    ty: String,
    parent_id: String,
    children_id: String,
    data: HashMap<String, Value>,
  ) -> Result<Block> {
    let block = Block {
      id: block_id.clone(),
      ty: BlockType::from_string(&ty),
      parent: parent_id,
      children: children_id,
      data: BlockDataEnum::from_map(BlockType::from_string(&ty), &data),
    };
    let block_map = self.root.insert_map_with_txn(txn, &block_id);
    block_map.insert_with_txn(txn, ID, block.id.clone());
    block_map.insert_with_txn(txn, TYPE, block.ty.to_string());
    block_map.insert_with_txn(txn, PARENT, block.parent.clone());
    block_map.insert_with_txn(txn, CHILDREN, block.children.clone());
    block_map.insert_with_txn(txn, DATA, block.data.to_string());
    Ok(block)
  }

  pub fn set_block_with_txn(&self, txn: &mut TransactionMut, block_id: &str, block: Block) {
    self.root.insert_json_with_txn(txn, block_id, block);
  }

  pub fn delete_block_with_txn(&self, txn: &mut TransactionMut, block_id: &str) {
    self.root.delete_with_txn(txn, block_id);
  }
}
