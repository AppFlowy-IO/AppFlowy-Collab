use collab::preclude::{CustomMapRef, MapRefWrapper, TransactionMut};
use collab_derive::Collab;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Collab, Serialize, Deserialize)]
pub struct Block {
  pub id: String,

  #[serde(rename = "type")]
  pub ty: String,

  pub next: String,

  #[serde(rename = "firstChild")]
  pub first_child: String,

  pub data: String,
}

impl Block {
  pub fn get_data<P: DataParser>(&self) -> Option<P::Output> {
    P::parser(&self.data)
  }
}

pub struct BlockMap {
  root: MapRefWrapper,
}
impl BlockMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn get_block(&self, block_id: &str) -> Option<BlockMapRef> {
    let txn = self.root.transact();
    let map_ref = self.root.get_map_with_txn(&txn, block_id)?;
    let block_map = BlockMapRef::from_map_ref(map_ref);
    drop(txn);
    Some(block_map)
  }

  pub fn create_block<B>(&self, block_id: &str, f: B)
  where
    B: FnOnce(BlockBuilder) -> BlockMapRef,
  {
    self.root.with_transact_mut(|txn| {
      let builder = BlockBuilder::new_with_txn(txn, block_id.to_string(), &self.root);
      let _ = f(builder);
    })
  }

  pub fn insert_block(&self, block: Block) {
    self.root.with_transact_mut(|txn| {
      self
        .root
        .insert_json_with_txn(txn, &block.id.clone(), block)
    })
  }
}

impl Deref for BlockMap {
  type Target = MapRefWrapper;

  fn deref(&self) -> &Self::Target {
    &self.root
  }
}

pub struct BlockBuilder<'a, 'b> {
  block_map: BlockMapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> BlockBuilder<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, container: &MapRefWrapper) -> Self {
    let key = nanoid!(4);
    Self::new_with_txn(txn, key, container)
  }

  pub fn new_with_txn(
    txn: &'a mut TransactionMut<'b>,
    block_id: String,
    container: &MapRefWrapper,
  ) -> Self {
    let map_ref = match container.get_map_with_txn(txn, &block_id) {
      None => container.insert_map_with_txn(txn, &block_id),
      Some(map) => map,
    };
    let block_map = BlockMapRef::from_map_ref(map_ref);

    Self { block_map, txn }
  }

  pub fn with_type<T: AsRef<str>>(mut self, ty: T) -> Self {
    self.block_map.set_ty(self.txn, ty.as_ref().to_string());
    self
  }

  pub fn with_data<T: ToString>(mut self, data: T) -> Self {
    self.block_map.set_data(self.txn, data.to_string());
    self
  }

  pub fn with_next<T: AsRef<str>>(mut self, next: T) -> Self {
    self.block_map.set_next(self.txn, next.as_ref().to_string());
    self
  }

  pub fn with_child<T: AsRef<str>>(mut self, child: T) -> Self {
    self
      .block_map
      .set_first_child(self.txn, child.as_ref().to_string());
    self
  }

  pub fn build(self) -> BlockMapRef {
    self.block_map
  }
}

pub trait DataParser {
  type Output;

  fn parser(data: &str) -> Option<Self::Output>;
}

pub struct TextDataParser {}

impl DataParser for TextDataParser {
  type Output = TextData;

  fn parser(data: &str) -> Option<Self::Output> {
    TextData::from_str(data).ok()
  }
}

#[derive(Serialize, Deserialize)]
pub struct TextData {
  pub text_id: String,
}

impl FromStr for TextData {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let object = serde_json::from_str(s)?;
    Ok(object)
  }
}

impl ToString for TextData {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}
