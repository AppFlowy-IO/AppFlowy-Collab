use crate::database::timestamp;
use crate::rows::{
  row_from_map_ref, row_from_value, row_order_from_value, BlockId, Cells, Row, RowBuilder, RowId,
  RowMetaMap, RowUpdate,
};
use crate::views::RowOrder;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::{
  Collab, CollabBuilder, Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use collab_persistence::CollabKV;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

const NUM_OF_BLOCKS: i64 = 10;

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
      let block_id = i;
      let collab = CollabBuilder::new(uid, format!("block_{}", block_id))
        .with_plugin(disk_plugin.clone())
        .build();
      collab.initial();

      let block = Block::new(block_id, collab);
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

  pub fn create_rows(&self, params: Vec<CreateRowParams>) -> Vec<RowOrder> {
    if params.is_empty() {
      return vec![];
    }

    let block_id = block_id_from_row_id(params[0].id);
    if let Some(block) = self.get_block(block_id) {
      let rows = params
        .into_iter()
        .map(|params| Row::from((block_id, params)))
        .collect::<Vec<Row>>();
      let row_orders = rows.iter().map(RowOrder::from).collect();
      block.insert_rows(rows);
      row_orders
    } else {
      dbg!("Can't find the block with block_id: {}", block_id);
      vec![]
    }
  }

  pub fn get_row(&self, row_id: RowId) -> Option<Row> {
    let block_id = block_id_from_row_id(row_id);
    if let Some(block) = self.get_block(block_id) {
      return block.get_row(row_id);
    }
    dbg!("Can't find the block with block_id: {}", block_id);
    None
  }

  pub fn create_row(&self, row: CreateRowParams) -> Option<RowOrder> {
    let block_id = block_id_from_row_id(row.id);
    if let Some(block) = self.get_block(block_id) {
      let row: Row = (block_id, row).into();
      let row_order: RowOrder = RowOrder::from(&row);

      println!("Insert row:{:?} to block:{:?}", row.id, block_id);
      block.create_row(row);
      Some(row_order)
    } else {
      dbg!("Can't find the block with block_id: {}", block_id);
      None
    }
  }

  pub fn remove_row(&self, row_id: RowId) {
    let block_id = block_id_from_row_id(row_id);
    if let Some(block) = self.get_block(block_id) {
      let row_id = row_id.to_string();
      block.delete_row(&row_id);
    }
  }

  pub fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      if let Some(block) = self.get_block(row_order.block_id) {
        let row = block.get_row(row_order.id);
        if let Some(row) = row {
          rows.push(row);
        }
      } else {
        dbg!("Can't find the block with block_id: {}", row_order.block_id);
      }
    }
    rows
  }

  pub fn update_row<F, R: Into<RowId>>(&self, row_id: R, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let row_id = row_id.into();
    let block_id = block_id_from_row_id(row_id);
    if let Some(block) = self.get_block(block_id) {
      block.update_row(row_id, f);
    } else {
      dbg!("Can't find the block with block_id: {}", block_id);
    }
  }
}

fn block_id_from_row_id(row_id: RowId) -> BlockId {
  let row_id: i64 = row_id.into();
  row_id % NUM_OF_BLOCKS
}

const BLOCK: &str = "block";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CreateRowParams {
  pub id: RowId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  pub prev_row_id: Option<RowId>,
}

impl CreateRowParams {
  pub fn new(id: RowId, cells: Cells, height: i32) -> Self {
    Self {
      id,
      cells,
      height,
      visibility: true,
      prev_row_id: None,
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

#[derive(Clone)]
pub struct Block {
  #[allow(dead_code)]
  collab: Collab,
  container: MapRefWrapper,
  pub block_id: BlockId,
  pub metas: RowMetaMap,
}

impl Block {
  fn new(block_id: BlockId, collab: Collab) -> Block {
    let (container, metas) = collab.with_transact_mut(|txn| {
      let block = collab
        .get_map_with_txn(txn, vec![BLOCK])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, BLOCK));

      let metas = RowMetaMap::new_with_txn(txn, &block);

      (block, metas)
    });

    Block {
      collab,
      container,
      block_id,
      metas,
    }
  }

  pub fn insert_rows(&self, row: Vec<Row>) {
    self.container.with_transact_mut(|txn| {
      for row in row {
        self.insert_row_with_txn(txn, row);
      }
    })
  }

  pub fn create_row<T: Into<Row>>(&self, row: T) {
    self.container.with_transact_mut(|txn| {
      self.insert_row_with_txn(txn, row);
    });
  }

  pub fn insert_row_with_txn<T: Into<Row>>(&self, txn: &mut TransactionMut, row: T) {
    let row = row.into();
    let row_id = row.id.to_string();
    let map_ref = self.container.insert_map_with_txn(txn, &row_id);
    RowBuilder::new(row.id, row.block_id, txn, map_ref)
      .update(|update| {
        update
          .set_height(row.height)
          .set_visibility(row.visibility)
          .set_created_at(row.created_at)
          .set_cells(row.cells);
      })
      .done();
  }

  pub fn get_row<R: Into<RowId>>(&self, row_id: R) -> Option<Row> {
    let txn = self.container.transact();
    self.get_row_with_txn(&txn, row_id)
  }

  pub fn get_row_with_txn<T: ReadTxn, R: Into<RowId>>(&self, txn: &T, row_id: R) -> Option<Row> {
    let row_id = row_id.into().to_string();
    let map_ref = self.container.get_map_with_txn(txn, &row_id)?;
    row_from_map_ref(&map_ref.into_inner(), txn)
  }

  pub fn get_all_rows(&self) -> Vec<Row> {
    let txn = self.container.transact();
    self.get_all_rows_with_txn(&txn)
  }

  pub fn get_all_rows_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Row> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| row_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn get_all_row_orders(&self) -> Vec<RowOrder> {
    let txn = self.container.transact();
    self.get_all_row_orders_with_txn(&txn)
  }

  pub fn get_all_row_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<RowOrder> {
    let mut ids = self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| row_order_from_value(v, txn))
      .collect::<Vec<(RowOrder, i64)>>();
    ids.sort_by(|(_, left), (_, right)| left.cmp(right));
    ids.into_iter().map(|(row_order, _)| row_order).collect()
  }

  pub fn delete_row(&self, row_id: &str) {
    self
      .container
      .with_transact_mut(|txn| self.container.delete_with_txn(txn, row_id));
  }

  pub fn update_row<F, R: Into<RowId>>(&self, row_id: R, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let row_id = row_id.into().to_string();
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_or_insert_map_with_txn(txn, &row_id);
      let update = RowUpdate::new(txn, &map_ref);
      f(update)
    })
  }
}

impl Deref for Block {
  type Target = RowMetaMap;

  fn deref(&self) -> &Self::Target {
    &self.metas
  }
}

impl DerefMut for Block {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.metas
  }
}
