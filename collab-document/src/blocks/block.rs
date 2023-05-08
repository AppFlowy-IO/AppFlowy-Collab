use crate::blocks::{hashmap_to_json_str, json_str_to_hashmap, Block, ChildrenOperation};
use crate::error::DocumentError;
use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub const EXTERNAL_TYPE_TEXT: &str = "text";
pub const EXTERNAL_TYPE_ARRAY: &str = "array";
pub const EXTERNAL_TYPE_MAP: &str = "map";

const ID: &str = "id";
const TYPE: &str = "ty";
const PARENT: &str = "parent";
const CHILDREN: &str = "children";
const DATA: &str = "data";
const EXTERNAL_ID: &str = "external_id";
const EXTERNAL_TYPE: &str = "external_type";

pub struct BlockOperation {
  root: MapRefWrapper,
  children_operation: Rc<ChildrenOperation>,
}

impl BlockOperation {
  pub fn new(root: MapRefWrapper, children_operation: Rc<ChildrenOperation>) -> Self {
    Self {
      root,
      children_operation,
    }
  }

  pub fn get_all_blocks(&self) -> HashMap<String, Block> {
    let txn = self.root.transact();
    self
      .root
      .iter(&txn)
      .filter_map(|(k, _)| {
        self
          .get_block_with_txn(&txn, k)
          .map(|block| (k.to_string(), block))
      })
      .collect()
  }

  fn get_block_from_root<T: ReadTxn>(&self, txn: &T, map: MapRefWrapper) -> Block {
    let id = map.get_str_with_txn(txn, ID).unwrap_or_default();
    let ty = map.get_str_with_txn(txn, TYPE).unwrap_or_default();
    let parent = map.get_str_with_txn(txn, PARENT).unwrap_or_default();
    let children = map.get_str_with_txn(txn, CHILDREN).unwrap_or_default();
    let json_str = map.get_str_with_txn(txn, DATA).unwrap_or_default();
    let data = json_str_to_hashmap(&json_str).unwrap_or_default();
    let external_id = map.get_str_with_txn(txn, EXTERNAL_ID);
    let external_type = map.get_str_with_txn(txn, EXTERNAL_TYPE);
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

  pub fn create_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    block: &Block,
  ) -> Result<Block, DocumentError> {
    let id = &block.id;
    if self.root.get_map_with_txn(txn, id).is_some() {
      return Err(DocumentError::BlockIsExistedAlready);
    }

    let map = self.root.insert_map_with_txn(txn, id);
    let data = &block.data;
    let json_str = hashmap_to_json_str(data.clone())?;

    map.insert_with_txn(txn, ID, id.to_string());
    map.insert_with_txn(txn, TYPE, block.ty.to_string());
    map.insert_with_txn(txn, PARENT, block.parent.to_string());
    map.insert_with_txn(txn, CHILDREN, block.children.to_string());
    map.insert_with_txn(txn, DATA, json_str);

    self
      .children_operation
      .get_children_with_txn(txn, &block.children);

    self
      .get_block_with_txn(txn, id)
      .ok_or(DocumentError::BlockCreateError)
  }

  pub fn delete_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
  ) -> Result<Block, DocumentError> {
    let block = self
      .get_block_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    self.root.remove(txn, id);
    self
      .children_operation
      .delete_children_with_txn(txn, &block.children);
    Ok(block)
  }

  pub fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block> {
    self
      .root
      .get_map_with_txn(txn, id)
      .map(|map| self.get_block_from_root(txn, map))
  }

  pub fn set_block_with_txn(
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
      map.insert_with_txn(txn, DATA, hashmap_to_json_str(data)?);
    }
    Ok(())
  }
}
