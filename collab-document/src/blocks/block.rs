use anyhow::Result;
use collab::preclude::{Map, MapRefWrapper, ReadTxn, TransactionMut};
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Deserialize, Debug)]
pub struct Block {
  pub id: String,

  pub ty: String,

  pub parent: String,

  pub children: String,

  pub data: String,
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
    let data = BlockDataEnum::from_string(&self.data);
    let data = match data {
      BlockDataEnum::Page(text) | BlockDataEnum::Text(text) => serde_json::json!({
        "text": text,
      }),
      BlockDataEnum::Header(level, text) => serde_json::json!({
        "level": level,
        "text": text,
      }),
      _ => serde_json::json!({}),
    };
    s.serialize_field("data", &data)?;

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
      let block = self.get_block(&txn, k).unwrap();
      let value = serde_json::to_value(block).unwrap();
      map.serialize_entry(k, &value)?;
    }
    map.end()
  }
}

impl BlockMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json(&self) -> serde_json::Value {
    serde_json::to_value(self).unwrap()
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
    data: BlockDataEnum,
  ) -> Result<Block> {
    let block = Block {
      id: block_id.clone(),
      ty,
      parent: parent_id,
      children: children_id,
      data: data.to_string(),
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
    self.root.insert_json_with_txn(txn, block_id, block);
  }

  pub fn delete_block_with_txn(&self, txn: &mut TransactionMut, block_id: &str) {
    self.root.remove(txn, block_id);
  }
}

#[derive(Serialize, Deserialize)]
pub enum BlockDataEnum {
  Page(String),
  Text(String),
  Header(u32, String),
  Image(),
}

impl ToString for BlockDataEnum {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap_or_else(|_| "".to_string())
  }
}

impl BlockDataEnum {
  pub fn from_string(s: &str) -> Self {
    serde_json::from_str(s).unwrap()
  }

  pub fn get_text(&self) -> Option<String> {
    match self {
      BlockDataEnum::Page(text) | BlockDataEnum::Text(text) => Some(text.clone()),
      BlockDataEnum::Header(_, text) => Some(text.clone()),
      _ => None,
    }
  }
}
