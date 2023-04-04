use std::collections::HashMap;

use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};

use crate::blocks::{ChildrenMap, TextMap};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::error::DocumentError;

const ID: &str = "id";
const TYPE: &str = "ty";
const PARENT: &str = "parent";
const CHILDREN: &str = "children";
const DATA: &str = "data";
const EXTERNAL_ID: &str = "external_id";
const EXTERNAL_TYPE: &str = "external_type";

pub const EXTERNAL_TYPE_TEXT: &str = "text";
pub const EXTERNAL_TYPE_ARRAY: &str = "array";
pub const EXTERNAL_TYPE_MAP: &str = "map";

pub trait OperableBlocks {
  fn create_block(&self, txn: &mut TransactionMut, block: &Block) -> Result<Block, DocumentError>;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub external_id: String,
  pub external_type: String,
  pub data: HashMap<String, Value>,
}

pub struct BlockMap {
  root: MapRefWrapper,
  pub children_map: ChildrenMap,
  pub text_map: TextMap,
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
      if let Ok(value) = serde_json::to_value(block) {
        s.serialize_entry(k, &value).unwrap();
      }
    });
    s.end()
  }
}

impl BlockMap {
  pub fn new(root: MapRefWrapper, children_map: ChildrenMap, text_map: TextMap) -> Self {
    Self {
      root,
      children_map,
      text_map,
    }
  }

  fn get_block_from_root<T: ReadTxn>(&self, txn: &T, map: MapRefWrapper) -> Block {
    let id = map.get_str_with_txn(txn, ID).unwrap_or_default();
    let ty = map.get_str_with_txn(txn, TYPE).unwrap_or_default();
    let parent = map.get_str_with_txn(txn, PARENT).unwrap_or_default();
    let children = map.get_str_with_txn(txn, CHILDREN).unwrap_or_default();
    let json_str = map.get_str_with_txn(txn, DATA).unwrap_or_default();
    let data = self.json_str_to_hashmap(&json_str).unwrap_or_default();
    let external_id = map.get_str_with_txn(txn, EXTERNAL_ID).unwrap_or_default();
    let external_type = map.get_str_with_txn(txn, EXTERNAL_TYPE).unwrap_or_default();
    Block {
      id,
      ty,
      parent,
      children,
      external_id,
      external_type,
      data,
    }
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
  fn create_block(&self, txn: &mut TransactionMut, block: &Block) -> Result<Block, DocumentError> {
    let id = &block.id;
    if self.root.get_map_with_txn(txn, id).is_some() {
      return Err(DocumentError::BlockIsExistedAlready);
    }
    let map = self.root.insert_map_with_txn(txn, id);
    let data = &block.data;
    let json_str = self.hashmap_to_json_str(data.clone())?;
    map.insert_with_txn(txn, ID, id.to_string());
    map.insert_with_txn(txn, TYPE, block.ty.to_string());
    map.insert_with_txn(txn, PARENT, block.parent.to_string());
    map.insert_with_txn(txn, CHILDREN, block.children.to_string());
    map.insert_with_txn(txn, DATA, json_str);
    map.insert_with_txn(txn, EXTERNAL_ID, block.external_id.to_string());
    map.insert_with_txn(txn, EXTERNAL_TYPE, block.external_type.to_string());

    if block.external_type == EXTERNAL_TYPE_TEXT {
      self.text_map.create_text(txn, &block.external_id);
    }
    self
      .children_map
      .get_children_with_txn(txn, &block.children);

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
    if block.external_type == EXTERNAL_TYPE_TEXT {
      self.text_map.delete_with_txn(txn, &block.external_id);
    }
    self
      .children_map
      .delete_children_with_txn(txn, &block.children);
    Ok(block)
  }

  fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block> {
    let map = self.root.get_map_with_txn(txn, id);
    map.map(|map| self.get_block_from_root(txn, map))
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
}
