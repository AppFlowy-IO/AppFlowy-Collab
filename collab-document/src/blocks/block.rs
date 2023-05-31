use crate::blocks::{hashmap_to_json_str, json_str_to_hashmap, Block, ChildrenOperation};
use crate::error::DocumentError;
use collab::preclude::{Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde_json::Value;
use std::collections::HashMap;

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

/// for block operate, there has a root map, and a children map.
pub struct BlockOperation {
  root: MapRefWrapper,
  children_operation: ChildrenOperation,
}

impl BlockOperation {
  pub fn new(root: MapRefWrapper, children_operation: ChildrenOperation) -> Self {
    Self {
      root,
      children_operation,
    }
  }

  /// get all blocks
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

  /// create a block
  pub fn create_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    block: Block,
  ) -> Result<Block, DocumentError> {
    if self.root.get_map_with_txn(txn, &block.id).is_some() {
      return Err(DocumentError::BlockAlreadyExists);
    }

    let block_id = block.id.clone();
    let children_id = block.children.clone();
    /// Create block map.
    let map = self.root.insert_map_with_txn(txn, &block.id);
    /// Generate data json string.
    let json_str = hashmap_to_json_str(block.data)?;

    /// Insert block fields.
    map.insert_with_txn(txn, ID, block.id);
    map.insert_with_txn(txn, TYPE, block.ty);
    map.insert_with_txn(txn, PARENT, block.parent);
    map.insert_with_txn(txn, CHILDREN, block.children);
    map.insert_with_txn(txn, DATA, json_str);

    /// Create the children for each block.
    self
      .children_operation
      .create_children_with_txn(txn, &children_id);

    /// Return the created block.
    self
      .get_block_with_txn(txn, &block_id)
      .ok_or(DocumentError::BlockCreateError)
  }

  /// Delete a block
  pub fn delete_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
  ) -> Result<Block, DocumentError> {
    let block = self
      .get_block_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    self.root.remove(txn, id);

    /// Delete the children for each block.
    self
      .children_operation
      .delete_children_with_txn(txn, &block.children);
    Ok(block)
  }

  /// Returns the block with the given id.
  pub fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block> {
    self
      .root
      .get_map_with_txn(txn, id)
      .map(|map| block_from_map(txn, map.into_inner()))
  }

  /**
   * Update the block with the given id.
   * Except `data` and `parent`, other fields can not be updated
   */
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

    /// Update parent field with the given parent id.
    if let Some(parent_id) = parent_id {
      map.insert_with_txn(txn, PARENT, parent_id);
    }
    /// Update data field with the given data.
    if let Some(data) = data {
      map.insert_with_txn(txn, DATA, hashmap_to_json_str(data)?);
    }
    Ok(())
  }
}

/// Build the block from the [MapRef]
fn block_from_map<T: ReadTxn>(txn: &T, map: MapRef) -> Block {
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
