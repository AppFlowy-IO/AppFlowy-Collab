use crate::blocks::{Block, BlockDataEnum, BlockMap, ChildrenMap, TextMap};
use crate::error::DocumentError;
use collab::preclude::*;
use nanoid::nanoid;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

const ROOT: &str = "document";
const BLOCKS: &str = "blocks";
const META: &str = "meta";
const TEXT_MAP: &str = "text_map";
const CHILDREN_MAP: &str = "children_map";

pub struct Document {
  #[allow(dead_code)]
  inner: Collab,
  root: MapRefWrapper,
  text_map: TextMap,
  children_map: ChildrenMap,
  pub blocks: BlockMap,
  pub meta: MapRefWrapper,
}

impl Serialize for Document {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let mut s = serializer.serialize_struct("Document", 3)?;
    s.serialize_field(
      "root_id",
      &self
        .root
        .get(&txn, "head_id")
        .unwrap_or_else(|| Value::from(""))
        .to_string(&txn),
    )?;
    s.serialize_field("blocks", &self.blocks.to_json())?;
    s.serialize_field(
      "meta",
      &serde_json::json!({
          "text_map": self.text_map.to_json(),
          "children_map": self.children_map.to_json(),
      }),
    )?;
    s.end()
  }
}

pub struct InsertBlockArgs {
  pub parent_id: String,
  pub block_id: String,
  pub data: BlockDataEnum,
  pub children_id: String,
  pub ty: String,
}

impl Document {
  pub fn create(collab: Collab) -> Self {
    let (root, blocks, meta, text_map, children_map) = collab.with_transact_mut(|txn| {
      // { document: {:} }
      let root = collab
        .get_map_with_txn(txn, vec![ROOT])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, ROOT));
      let head_id = nanoid!();
      // { document: { head_id: "uuid" } }
      root.insert_with_txn(txn, "head_id", head_id);
      // { document: { blocks: {:} } }
      let blocks = collab
        .get_map_with_txn(txn, vec![ROOT, BLOCKS])
        .unwrap_or_else(|| root.insert_map_with_txn(txn, BLOCKS));
      // { document: { blocks: {:}, meta: {:} } }
      let meta = collab
        .get_map_with_txn(txn, vec![ROOT, META])
        .unwrap_or_else(|| root.insert_map_with_txn(txn, META));
      // { document: { blocks: {:}, meta: { text_map: {:} } }
      let text_map = collab
        .get_map_with_txn(txn, vec![META, TEXT_MAP])
        .unwrap_or_else(|| meta.insert_map_with_txn(txn, TEXT_MAP));
      // {document: { blocks: {:}, meta: { text_map: {:}, children_map: {:} } }
      let children_map = collab
        .get_map_with_txn(txn, vec![META, CHILDREN_MAP])
        .unwrap_or_else(|| meta.insert_map_with_txn(txn, CHILDREN_MAP));

      (root, blocks, meta, text_map, children_map)
    });
    let blocks = BlockMap::new(blocks);
    let text_map = TextMap::new(text_map);
    let children_map = ChildrenMap::new(children_map);

    let document = Self {
      inner: collab,
      root,
      blocks,
      meta,
      text_map,
      children_map,
    };
    document.inner.with_transact_mut(|txn| {
      document.init(txn);
    });

    document
  }

  pub fn to_json(&self) -> Result<serde_json::value::Value, DocumentError> {
    let document_data = serde_json::json!({
        "document": serde_json::to_value(self).unwrap()
    });

    Ok(document_data)
  }

  pub fn init(&self, txn: &mut TransactionMut) {
    let head_id = self.root.get(txn, "head_id").unwrap().to_string(txn);
    let head_children_id = nanoid!();
    let head_text_id = nanoid!();
    let head_data = BlockDataEnum::Page(head_text_id);
    // { document: { blocks: { head_id: { id: "head_id", ty: "page", data: { text: "head_text_id", level: null }, children: "head_children_id" } } } }
    self.insert_block(
      txn,
      InsertBlockArgs {
        parent_id: "".to_string(),
        block_id: head_id.clone(),
        data: head_data,
        children_id: head_children_id,
        ty: "page".to_string(),
      },
      "".to_string(),
    );

    let first_id = nanoid!();
    let first_text_id = nanoid!();
    let first_children_id = nanoid!();
    let first_data = BlockDataEnum::Text(first_text_id);
    // { document: { blocks: { head_id: { id: "head_id", ty: "page", data: { text: "head_text_id", level: null }, children: "head_children_id" }, first_id: { id: "first_id", ty: "text", data: { text: "first_text_id", level: null }, children: "first_children_id" } } } }
    self.insert_block(
      txn,
      InsertBlockArgs {
        parent_id: head_id,
        block_id: first_id,
        data: first_data,
        children_id: first_children_id,
        ty: "text".to_string(),
      },
      "".to_string(),
    );
  }

  pub fn with_txn(&self, f: impl FnOnce(&mut TransactionMut)) {
    self.inner.with_transact_mut(f);
  }

  pub fn get_block(&self, block_id: &str) -> Option<Block> {
    let txn = self.inner.transact();
    self.blocks.get_block(&txn, block_id)
  }

  pub fn insert_block(&self, txn: &mut TransactionMut, block: InsertBlockArgs, prev_id: String) {
    let block_id = block.block_id;
    let ty = block.ty;
    let parent_id = block.parent_id;
    let children_id = block.children_id;
    let text_id = block.data.get_text();

    self
      .children_map
      .create_children_with_txn(txn, children_id.clone());

    if let Some(text_id) = text_id {
      self.text_map.create_text(txn, &text_id);
    }

    let block = self
      .blocks
      .create_block(txn, block_id, ty, parent_id, children_id, block.data);

    match block {
      Ok(block) => self.insert_block_to_parent(txn, &block, prev_id),
      _ => {
        println!("block create fail!");
      },
    };
  }

  pub fn insert_block_to_parent(&self, txn: &mut TransactionMut, block: &Block, prev_id: String) {
    let parent_id = &block.parent;
    if parent_id.is_empty() {
      return;
    }
    let parent = self.blocks.get_block(txn, parent_id);

    let parent_is_empty = parent.is_none();
    if parent_is_empty {
      return;
    }

    let parent = parent.unwrap();
    let parent_children_id = &parent.children;
    let mut index = 0;
    if !prev_id.is_empty() {
      let prev_index =
        self
          .children_map
          .get_child_index_with_txn(txn, parent_children_id, &prev_id);
      match prev_index {
        Some(prev_index) => {
          index = prev_index + 1;
        },
        None => {
          index = 0;
        },
      }
    }
    self
      .children_map
      .insert_child_with_txn(txn, parent_children_id, &block.id, index);
  }

  pub fn delete_block(&self, txn: &mut TransactionMut, block_id: &str) {
    let block = self.blocks.get_block(txn, block_id);
    if block.is_none() {
      return;
    }

    let block = block.unwrap();
    let children_id = &block.children;
    let block_data = BlockDataEnum::from_string(&block.data);

    let parent_id = &block.parent;
    self.delete_block_from_parent(txn, block_id, parent_id);

    self.children_map.delete_children_with_txn(txn, children_id);

    let text_id = block_data.get_text();

    if let Some(text_id) = text_id {
      self.text_map.delete_with_txn(txn, &text_id);
    }
    self.blocks.delete_block_with_txn(txn, block_id);
  }

  pub fn delete_block_from_parent(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    parent_id: &str,
  ) {
    let parent = self.blocks.get_block(txn, parent_id);

    if let Some(parent) = parent {
      let parent_children_id = &parent.children;
      self
        .children_map
        .delete_child_with_txn(txn, parent_children_id, block_id);
    }
  }

  pub fn move_block(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    parent_id: &str,
    prev_id: &str,
  ) {
    let block = self.blocks.get_block(txn, block_id);
    if block.is_none() {
      return;
    }
    let block = block.unwrap();
    let parent = self.blocks.get_block(txn, parent_id);
    if parent.is_none() {
      return;
    }

    let parent = parent.unwrap();
    let old_parent = self.blocks.get_block(txn, &block.parent);
    if old_parent.is_none() {
      return;
    }

    let old_parent = old_parent.unwrap();
    let old_parent_children_id = old_parent.children;
    let new_parent_children_id = parent.children;

    let prev_index =
      self
        .children_map
        .get_child_index_with_txn(txn, &new_parent_children_id, prev_id);

    let new_index = match prev_index {
      Some(prev_index) => prev_index + 1,
      None => 0,
    };

    self
      .children_map
      .delete_child_with_txn(txn, &old_parent_children_id, block_id);
    self
      .children_map
      .insert_child_with_txn(txn, &new_parent_children_id, block_id, new_index);

    self.blocks.set_block_with_txn(
      txn,
      block_id,
      Block {
        parent: parent_id.to_string(),
        ..block
      },
    );
  }
}
