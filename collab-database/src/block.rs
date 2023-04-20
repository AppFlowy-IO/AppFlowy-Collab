use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

use collab::plugin_impl::sled_disk::SledDiskPlugin;
use collab::preclude::{
  Collab, CollabBuilder, Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use collab_persistence::kv::sled_lv::SledCollabDB;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab_persistence::kv::rocks_kv::RocksCollabDB;

use crate::database::timestamp;
use crate::rows::{
  cell_from_map_ref, create_row_meta, get_row_meta, row_from_map_ref, row_from_value,
  row_order_from_value, BlockId, Cell, Cells, Row, RowBuilder, RowId, RowMetaMap, RowUpdate,
};
use crate::views::RowOrder;

const NUM_OF_BLOCKS: i64 = 10;

/// It's used to store the blocks. Each [Block] is indexed by the block_id.
#[derive(Clone)]
pub struct Blocks {
  pub blocks: Rc<RwLock<HashMap<BlockId, Rc<Block>>>>,
}

impl Blocks {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>) -> Self {
    let blocks = RwLock::new(HashMap::new());
    let mut write_guard = blocks.write();

    // Create the [NUM_OF_BLOCKS] blocks if it's not exist.
    for i in 0..NUM_OF_BLOCKS {
      let block_id = i;
      let collab = CollabBuilder::new(uid, format!("block_{}", block_id))
        .with_plugin(RocksDiskPlugin::new(uid, db.clone()).unwrap())
        .build();
      collab.initial();

      // Create a new [Block] with the given id.
      let block = Block::new(block_id, collab);
      write_guard.insert(block_id, Rc::new(block));
    }
    drop(write_guard);
    Self {
      blocks: Rc::new(blocks),
    }
  }

  /// Return the block with the given block_id.
  /// If the block is not exist, return None.
  pub fn get_block<T: Into<BlockId>>(&self, block_id: T) -> Option<Rc<Block>> {
    let block_id = block_id.into();
    let blocks = self.blocks.read();
    blocks.get(&block_id).cloned()
  }

  /// Create the given rows from the given [CreateRowParams]s.
  /// Return the [RowOrder]s.
  /// A row will be stored in the corresponding block base on its [RowId]. The rows stored
  /// in the block are not ordered. We use the [RowOrder]s to keep track of the order of the rows.
  /// The [RowOrder]s are stored in the [DatabaseView].
  pub fn create_rows(&self, create_row_params: Vec<CreateRowParams>) -> Vec<RowOrder> {
    if create_row_params.is_empty() {
      return vec![];
    }
    let mut row_orders: Vec<RowOrder> = vec![];
    let mut block_rows: HashMap<BlockId, Vec<CreateRowParams>> = HashMap::new();
    for params in create_row_params {
      // Get the block id base on the row id.
      let block_id = block_id_from_row_id(params.id);
      row_orders.push(RowOrder {
        id: params.id,
        block_id,
        height: params.height,
      });

      block_rows
        .entry(block_id)
        .or_insert_with(std::vec::Vec::new)
        .push(params);
    }

    for (block_id, params) in block_rows.into_iter() {
      if let Some(block) = self.get_block(block_id) {
        let rows = params
          .into_iter()
          .map(|params| Row::from((block_id, params)))
          .collect::<Vec<Row>>();
        block.insert_rows(rows);
      }
    }

    row_orders
  }

  /// Return the [Row] with the given [RowId].
  /// If the row is not exist, return None.
  pub fn get_row(&self, row_id: RowId) -> Option<Row> {
    let block_id = block_id_from_row_id(row_id);
    if let Some(block) = self.get_block(block_id) {
      return block.get_row(row_id);
    }
    dbg!("Can't find the block with block_id: {}", block_id);
    None
  }

  /// Create a new row with the given [CreateRowParams].
  /// Return the [RowOrder] of the new row.
  pub fn create_row(&self, row: CreateRowParams) -> Option<RowOrder> {
    let block_id = block_id_from_row_id(row.id);
    if let Some(block) = self.get_block(block_id) {
      let row: Row = (block_id, row).into();
      let row_order: RowOrder = RowOrder::from(&row);

      tracing::trace!("Insert row:{:?} to block:{:?}", row.id, block_id);
      block.create_row(row);
      Some(row_order)
    } else {
      dbg!("Can't find the block with block_id: {}", block_id);
      None
    }
  }

  /// Remove the row with the given [RowId].
  pub fn remove_row(&self, row_id: RowId) {
    let block_id = block_id_from_row_id(row_id);
    if let Some(block) = self.get_block(block_id) {
      let row_id = row_id.to_string();
      block.delete_row(&row_id);
    }
  }

  /// Return the [Cell] with the given [RowId] and field_id.
  pub fn get_cell(&self, field_id: &str, row_id: RowId) -> Option<Cell> {
    let block_id = block_id_from_row_id(row_id);
    self
      .blocks
      .read()
      .get(&block_id)?
      .get_cell(row_id, field_id)
  }

  /// Return the [Row]s with the given [RowOrder]s.
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

  /// Update the row with the given [RowId] and [RowUpdate].
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

/// Return the block id base on the row id.
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prev_row_id: Option<RowId>,
  pub timestamp: i64,
}

impl CreateRowParams {
  pub fn new(id: RowId) -> Self {
    Self {
      id,
      cells: Cells::default(),
      height: 60,
      visibility: true,
      prev_row_id: None,
      timestamp: timestamp(),
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
      created_at: params.timestamp,
    }
  }
}

pub struct Block {
  #[allow(dead_code)]
  collab: Collab,
  container: MapRefWrapper,
  pub block_id: BlockId,
  pub metas: RowMetaMap,
}

impl Block {
  fn new(block_id: BlockId, collab: Collab) -> Block {
    let (block, meta) = {
      let txn = collab.transact();
      let block = collab.get_map_with_txn(&txn, vec![BLOCK]);
      let meta = match &block {
        None => None,
        Some(block) => get_row_meta(&txn, block),
      };
      (block, meta)
    };

    match block {
      None => {
        let (block, meta) = collab.with_transact_mut(|txn| {
          let block = collab.create_map_with_txn(txn, BLOCK);
          let meta = create_row_meta(txn, &block);
          (block, meta)
        });
        Block {
          collab,
          container: block,
          block_id,
          metas: RowMetaMap::new(meta),
        }
      },
      Some(block) => {
        let meta = match meta {
          None => collab.with_transact_mut(|txn| create_row_meta(txn, &block)),
          Some(meta) => meta,
        };
        Block {
          collab,
          container: block,
          block_id,
          metas: RowMetaMap::new(meta),
        }
      },
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

  pub fn get_cell<R: Into<RowId>>(&self, row_id: R, field_id: &str) -> Option<Cell> {
    let txn = self.container.transact();
    let row_id = row_id.into().to_string();
    let map_ref = self.container.get_map_with_txn(&txn, &row_id)?;
    cell_from_map_ref(&map_ref.into_inner(), &txn, field_id)
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
