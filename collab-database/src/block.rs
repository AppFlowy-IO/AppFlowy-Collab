use crate::database::timestamp;
use crate::rows::{BlockId, Cells, Row, RowId, RowMap};
use crate::views::RowOrder;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::{Collab, CollabBuilder, ReadTxn, TransactionMut};
use collab_persistence::CollabKV;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

const NUM_OF_BLOCKS: i64 = 1;

#[derive(Clone)]
pub struct Blocks {
  pub blocks: Rc<RwLock<HashMap<BlockId, Rc<Block>>>>,
}

impl Blocks {
  pub fn new(uid: i64, db: Arc<CollabKV>) -> Self {
    let blocks = RwLock::new(HashMap::new());
    let disk_plugin = CollabDiskPlugin::new(uid, db).unwrap();
    let mut write_guard = blocks.write();
    for i in 0..NUM_OF_BLOCKS {
      let block_id = BlockId::from(i);
      let collab = CollabBuilder::new(uid, format!("block_{}", block_id))
        .with_plugin(disk_plugin.clone())
        .build();
      collab.initial();

      let block = create_block(block_id, collab);
      write_guard.insert(block_id, Rc::new(block));
    }
    drop(write_guard);
    Self {
      blocks: Rc::new(blocks),
    }
  }

  pub fn get_block<T: Into<BlockId>>(&self, block_id: T) -> Option<Rc<Block>> {
    let block_id = block_id.into();
    let blocks = self.blocks.read();
    blocks.get(&block_id).cloned()
  }

  pub fn create_rows(&self, params: Vec<CreateRowParams>) {
    let row_id: i64 = params.id.into();
    let block_id = row_id % NUM_OF_BLOCKS;

    if let Some(block) = self.get_block(block_id) {
      let rows = params
        .into_iter()
        .map(|params| Row::from((block_id, params)))
        .collect();
      let row = Row::from((block_id, params));
      block.insert_row(txn, rows);
    }
  }

  pub fn get_row_with_txn(
    &self,
    txn: &mut TransactionMut,
    row_id: RowId,
    block_id: BlockId,
  ) -> Option<Row> {
    if let Some(block) = self.get_block(block_id) {
      return block.get_row_with_txn(txn, row_id);
    }
    dbg!("Can't find the block with block_id: {}", block_id);
    None
  }

  pub fn insert_row_with_txn(&self, txn: &mut TransactionMut, row: Row) {
    let block_id = row.block_id;
    if let Some(block) = self.get_block(block_id) {
      block.insert_row_with_txn(txn, row);
    } else {
      dbg!("Can't find the block with block_id: {}", block_id);
    }
  }

  pub fn remove_row_with_txn(&self, txn: &mut TransactionMut, row_id: RowId, block_id: BlockId) {
    if let Some(block) = self.get_block(block_id) {
      let row_id = row_id.to_string();
      block.delete_row_with_txn(txn, &row_id);
    }
  }

  pub fn get_rows_from_row_orders<T: ReadTxn>(&self, txn: &T, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      if let Some(block) = self.get_block(row_order.block_id) {
        let row = block.get_row_with_txn(txn, row_order.id);
        if let Some(row) = row {
          rows.push(row);
        }
      } else {
        dbg!("Can't find the block with block_id: {}", row_order.block_id);
      }
    }
    rows
  }
}

const BLOCK: &str = "block";
fn create_block(block_id: BlockId, collab: Collab) -> Block {
  let rows = collab.with_transact_mut(|txn| {
    let block = collab
      .get_map_with_txn(txn, vec![BLOCK])
      .unwrap_or_else(|| collab.create_map_with_txn(txn, BLOCK));
    RowMap::new_with_txn(txn, block)
  });

  Block {
    collab,
    block_id,
    rows,
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRowParams {
  pub id: RowId,
  pub block_id: BlockId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
}

impl CreateRowParams {
  pub fn new(id: RowId, block_id: BlockId, cells: Cells, height: i32) -> Self {
    Self {
      id,
      block_id,
      cells,
      height,
      visibility: true,
    }
  }
}

impl From<(BlockId, CreateRowParams)> for Row {
  fn from(params: (BlockId, CreateRowParams)) -> Self {
    let (block_id, params) = params;
    Row {
      id: params.id,
      block_id,
      cells: params.cells,
      height: params.height,
      visibility: params.visibility,
      created_at: timestamp(),
    }
  }
}

impl From<&CreateRowParams> for RowOrder {
  fn from(params: &CreateRowParams) -> Self {
    Self {
      id: params.id,
      block_id: params.block_id,
      height: params.height,
    }
  }
}

#[derive(Clone)]
pub struct Block {
  collab: Collab,
  pub block_id: BlockId,
  pub rows: RowMap,
}

impl Deref for Block {
  type Target = RowMap;

  fn deref(&self) -> &Self::Target {
    &self.rows
  }
}

impl DerefMut for Block {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.rows
  }
}
