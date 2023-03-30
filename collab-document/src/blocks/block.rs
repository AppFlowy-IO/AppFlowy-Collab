use anyhow::Result;
use collab::preclude::{Map, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Deserialize, Serialize, Debug)]
pub struct Block {
  pub id: String,

  pub ty: String,

  pub parent: String,

  pub children: String,

  pub data: String,
}

pub struct BlockMap {
  root: MapRefWrapper,
}

impl BlockMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json(&self) -> serde_json::Value {
    let mut obj = serde_json::json!({});
    let txn = self.root.transact();
    self.root.iter(&txn).for_each(|(key, _)| {
      let key = key.to_string();
      let block = self.get_block(&txn, &key);
      match block {
        Some(block) => {
          let block = serde_json::json!({
              "id": block.id,
              "ty": block.ty,
              "parent": block.parent,
              "children": block.children,
              "data": BlockDataParser::parser(&block.data),
          });
          obj[key] = block;
        },
        None => {},
      }
    });
    obj
  }

  pub fn get_block<T: ReadTxn>(&self, txn: &T, block_id: &str) -> Option<Block> {
    let block_map = self.root.get_map_with_txn(txn, block_id);
    match block_map {
      Some(block_map) => {
        let block = self.get_block_by_map(txn, block_map);
        Some(block)
      },
      None => None,
    }
  }

  pub fn get_block_by_map<T: ReadTxn>(&self, txn: &T, block_map: MapRefWrapper) -> Block {
    let id = block_map.get(txn, "id").unwrap().to_string(txn);
    let ty = block_map.get(txn, "ty").unwrap().to_string(txn);
    let parent = block_map.get(txn, "parent").unwrap().to_string(txn);
    let children = block_map.get(txn, "children").unwrap().to_string(txn);
    let data = block_map.get(txn, "data").unwrap().to_string(txn);
    Block {
      id,
      ty,
      parent,
      children,
      data,
    }
  }

  pub fn create_block(
    &self,
    txn: &mut TransactionMut,
    block_id: String,
    ty: String,
    parent_id: String,
    children_id: String,
    data: BlockData,
  ) -> Result<Block> {
    let block = Block {
      id: block_id.clone(),
      ty,
      parent: parent_id,
      children: children_id,
      data: BlockData::to_string(&data),
    };
    let block_map = self.root.insert_map_with_txn(txn, &block_id);
    block_map.insert_with_txn(txn, "id", block.id.clone());
    block_map.insert_with_txn(txn, "ty", block.ty.clone());
    block_map.insert_with_txn(txn, "parent", block.parent.clone());
    block_map.insert_with_txn(txn, "children", block.children.clone());
    block_map.insert_with_txn(txn, "data", block.data.clone());
    Ok(block)
  }

  pub fn set_block_with_txn(&self, txn: &mut TransactionMut, block_id: &str, block: Block) {
    self.root.insert_json_with_txn(txn, &block_id, block);
  }

  pub fn delete_block_with_txn(&self, txn: &mut TransactionMut, block_id: &str) {
    self.root.remove(txn, block_id);
  }
}

pub trait DataParser {
  type Output;

  fn parser(data: &str) -> Option<Self::Output>;

  fn to_string(data: &Self::Output) -> String;
}

pub struct BlockDataParser {}

impl DataParser for BlockDataParser {
  type Output = BlockData;

  fn parser(data: &str) -> Option<Self::Output> {
    BlockData::from_str(data).ok()
  }

  fn to_string(data: &Self::Output) -> String {
    BlockData::to_string(data)
  }
}

#[derive(Serialize, Deserialize)]
pub struct BlockData {
  pub text: String,
  pub level: Option<u32>,
}

impl FromStr for BlockData {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let object = serde_json::from_str(s)?;
    Ok(object)
  }
}

impl ToString for BlockData {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}
