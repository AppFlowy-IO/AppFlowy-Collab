use crate::blocks::{hashmap_to_json_str, json_str_to_hashmap, ChildrenOperation, TextOperation};
use crate::error::DocumentError;
use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
  pub id: String,
  pub ty: String,
  pub parent: String,
  pub children: String,
  pub external_id: Option<String>,
  pub external_type: Option<String>,
  pub data: HashMap<String, Value>,
}

pub struct BlockOperation {
  pub root: MapRefWrapper,
  children_operation: Rc<ChildrenOperation>,
  text_operation: Rc<TextOperation>,
}

impl Serialize for BlockOperation {
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

impl BlockOperation {
  pub fn new(
    root: MapRefWrapper,
    children_operation: Rc<ChildrenOperation>,
    text_operation: Rc<TextOperation>,
  ) -> Self {
    Self {
      root,
      children_operation,
      text_operation,
    }
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

    if let (Some(external_type), Some(external_id)) =
      (block.external_type.clone(), block.external_id.clone())
    {
      map.insert_with_txn(txn, EXTERNAL_TYPE, external_type.to_string());
      map.insert_with_txn(txn, EXTERNAL_ID, external_id.to_string());
      if external_type == EXTERNAL_TYPE_TEXT {
        self.text_operation.create_text(txn, &external_id);
      }
    }

    self
      .children_operation
      .get_children_with_txn(txn, &block.children);

    Ok(self.get_block_with_txn(txn, id).unwrap())
  }

  pub fn delete_block_with_txn(
    &self,
    txn: &mut TransactionMut,
    id: &str,
  ) -> Result<Block, DocumentError> {
    let block = self
      .get_block_with_txn(txn, id)
      .ok_or(DocumentError::BlockIsNotFound)?;
    self.root.remove_with_txn(txn, id);

    if let (Some(external_type), Some(external_id)) =
      (block.external_type.clone(), block.external_id.clone())
    {
      if external_type == EXTERNAL_TYPE_TEXT {
        self.text_operation.delete_with_txn(txn, &external_id);
      }
    }

    self
      .children_operation
      .delete_children_with_txn(txn, &block.children);
    Ok(block)
  }

  pub fn get_block_with_txn<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<Block> {
    let map = self.root.get_map_with_txn(txn, id);
    map.map(|map| self.get_block_from_root(txn, map))
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
