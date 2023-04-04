use std::collections::HashMap;

use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};

use serde::{
  ser::{SerializeMap, SerializeStruct},
  Deserialize, Serialize, Serializer,
};
use serde_json::Value;

use crate::error::DocumentError;

const ID: &str = "id";
const TYPE: &str = "ty";
const PARENT: &str = "parent";
const CHILDREN: &str = "children";
const DATA: &str = "data";

pub trait OperableBlocks {
  fn create_block(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    ty: &str,
    parent: &str,
    children: &str,
    data: HashMap<String, Value>,
  ) -> Result<Block, DocumentError>;

  fn delete_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
  ) -> Result<Block, DocumentError>;

  fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block>;

  fn set_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    data: Option<HashMap<String, Value>>,
    parent_id: Option<&str>,
  ) -> Result<(), DocumentError>;
}

#[derive(Deserialize, Debug)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub data: HashMap<String, Value>,
}

impl Serialize for Block {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state = serializer.serialize_struct("Block", 5)?;
    state.serialize_field(ID, &self.id)?;
    state.serialize_field(TYPE, &self.ty)?;
    state.serialize_field(PARENT, &self.parent)?;
    state.serialize_field(CHILDREN, &self.children)?;
    state.serialize_field(DATA, &self.data)?;
    state.end()
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
    let len = self.root.len(&txn) as usize;
    let mut s = serializer.serialize_map(Some(len))?;
    self.root.iter(&txn).for_each(|(k, _)| {
      let block = self.get_block_with_txn(&txn, k).unwrap();
      match serde_json::to_value(block) {
        Ok(value) => _ = s.serialize_entry(k, &value),
        Err(_) => (),
      }
    });
    s.end()
  }
}

impl BlockMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  fn get_block_from_root<T: ReadTxn>(&self, txn: &T, map: MapRefWrapper) -> Option<Block> {
    let id = map.get_str_with_txn(txn, ID)?;
    let ty = map.get_str_with_txn(txn, TYPE)?;
    let parent = map.get_str_with_txn(txn, PARENT)?;
    let children = map.get_str_with_txn(txn, CHILDREN)?;
    let json_str = map.get_str_with_txn(txn, DATA)?;
    let data = self.json_str_to_hashmap(&json_str).ok()?;

    Some(Block {
      id,
      ty,
      parent,
      children,
      data,
    })
  }

  fn json_str_to_hashmap(&self, json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
    let v = serde_json::from_str(json_str);
    v.map_err(|_| DocumentError::ConvertDataError)
  }

  fn hashmap_to_json_str(&self, data: HashMap<String, Value>) -> Result<String, DocumentError> {
    let v = serde_json::to_string(&data);
    v.map_err(|_| DocumentError::ConvertDataError)
  }
}

impl OperableBlocks for BlockMap {
  fn create_block(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    ty: &str,
    parent: &str,
    children: &str,
    data: HashMap<String, Value>,
  ) -> Result<Block, DocumentError> {
    if self.root.get_map_with_txn(txn, id).is_some() {
      return Err(DocumentError::BlockIsExistedAlready);
    }
    let map = self.root.insert_map_with_txn(txn, id);
    map.insert_with_txn(txn, ID, id);
    map.insert_with_txn(txn, TYPE, ty);
    map.insert_with_txn(txn, PARENT, parent);
    map.insert_with_txn(txn, CHILDREN, children);
    map.insert_with_txn(txn, DATA, self.hashmap_to_json_str(data)?);
    Ok(self.get_block_with_txn(txn, id).unwrap())
  }

  fn delete_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
  ) -> Result<Block, DocumentError> {
    let block = self
      .get_block_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    self.root.remove_with_txn(txn, id);
    Ok(block)
  }

  fn set_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    data: Option<HashMap<String, Value>>,
    parent_id: Option<&str>,
  ) -> Result<(), DocumentError> {
    let map = self
      .root
      .get_map_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    if let Some(parent_id) = parent_id {
      map.insert_with_txn(txn, PARENT, parent_id);
    }
    if let Some(data) = data {
      map.insert_with_txn(txn, DATA, self.hashmap_to_json_str(data)?);
    }
    Ok(())
  }

  fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block> {
    let map = self.root.get_map_with_txn(txn, id)?;
    self.get_block_from_root(txn, map)
  }
}
