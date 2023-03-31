use std::collections::HashMap;

use collab::preclude::{Map, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{
  ser::{SerializeMap, SerializeStruct},
  Deserialize, Serialize, Serializer,
};
use serde_json::Value;

use crate::{document::Document, error::DocumentError};

const BLOCK_ID: &str = "id";
const BLOCK_TYPE: &str = "ty";
const BLOCK_PARENT: &str = "parent";
const BLOCK_CHILDREN: &str = "children";
const BLOCK_DATA: &str = "data";

#[derive(Deserialize, Debug)]
pub struct BlockV2 {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub data: HashMap<String, Value>,
}

impl Serialize for BlockV2 {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state = serializer.serialize_struct("Block", 5)?;
    state.serialize_field(BLOCK_ID, &self.id)?;
    state.serialize_field(BLOCK_TYPE, &self.ty)?;
    state.serialize_field(BLOCK_PARENT, &self.parent)?;
    state.serialize_field(BLOCK_CHILDREN, &self.children)?;
    state.serialize_field(BLOCK_DATA, &self.data)?;
    state.end()
  }
}

/// {
///   "block_map": root,
/// }
pub struct BlockMapV2 {
  root: MapRefWrapper,
}

impl Serialize for BlockMapV2 {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let len = self.root.len(&txn) as usize;
    let mut s = serializer.serialize_map(Some(len))?;
    self
      .root
      .iter(&txn)
      .for_each(|(k, _)| match self.get_block(&txn, k) {
        Some(block) => {
          let value = serde_json::to_value(block).unwrap();
          s.serialize_entry(k, &value).unwrap();
        },
        None => (),
      });
    s.end()
  }
}

impl BlockMapV2 {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn create_block(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    ty: &str,
    parent: &str,
    children: &str,
    data: HashMap<String, Value>,
  ) -> Result<(), DocumentError> {
    if self.root.get_map_with_txn(txn, id).is_some() {
      return Err(DocumentError::BlockIsExistedAlready);
    }
    let map = self.root.insert_map_with_txn(txn, id);
    map.insert_with_txn(txn, BLOCK_ID, id);
    map.insert_with_txn(txn, BLOCK_TYPE, ty);
    map.insert_with_txn(txn, BLOCK_PARENT, parent);
    map.insert_with_txn(txn, BLOCK_CHILDREN, children);
    map.insert_with_txn(txn, BLOCK_DATA, serde_json::to_string(&data).unwrap());
    Ok(())
  }

  pub fn set_block_with_txn<T: ReadTxn>(
    &self,
    txn: &mut TransactionMut,
    id: &str,
    data: HashMap<String, Value>,
  ) -> Result<(), DocumentError> {
    let map = self
      .root
      .get_map_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    Ok(map.insert_with_txn(txn, BLOCK_DATA, serde_json::to_string(&data).unwrap()))
  }

  pub fn get_block_from_map<T: ReadTxn>(&self, txn: &T, map: MapRefWrapper) -> Option<BlockV2> {
    let id = map.get_str_with_txn(txn, BLOCK_ID)?;
    let ty = map.get_str_with_txn(txn, BLOCK_TYPE)?;
    let parent = map.get_str_with_txn(txn, BLOCK_PARENT)?;
    let children = map.get_str_with_txn(txn, BLOCK_CHILDREN)?;
    let json_str = map.get_str_with_txn(txn, BLOCK_DATA)?;
    let data = Self::json_str_to_hashmap(&json_str).ok()?;

    Some(BlockV2 {
      id,
      ty,
      parent,
      children,
      data,
    })
  }

  pub fn get_block<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<BlockV2> {
    let map = self.root.get_map_with_txn(txn, id)?;
    self.get_block_from_map(txn, map)
  }

  fn json_str_to_hashmap(json_str: &str) -> Result<HashMap<String, Value>, DocumentError> {
    let v = serde_json::from_str(json_str);
    v.map_err(|_| DocumentError::ConvertDataError)
  }
}
