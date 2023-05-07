use std::collections::HashMap;
use std::rc::Rc;

use crate::blocks::{
  Block, BlockAction, BlockActionType, BlockEvent, BlockOperation, ChildrenOperation, DocumentData,
  DocumentMeta, RootDeepSubscription,
};
use crate::error::DocumentError;
use collab::preclude::*;
use serde_json::Value;

const ROOT: &str = "document";
const PAGE_ID: &str = "page_id";
const BLOCKS: &str = "blocks";
const META: &str = "meta";
const CHILDREN_MAP: &str = "children_map";

pub struct Document {
  #[allow(dead_code)]
  inner: Collab,
  root: MapRefWrapper,
  #[allow(dead_code)]
  subscription: RootDeepSubscription,
  children_operation: Rc<ChildrenOperation>,
  block_operation: BlockOperation,
}

impl Document {
  // Create or get a document.
  pub fn create(collab: Collab) -> Result<Document, DocumentError> {
    let is_exist = {
      let txn = &collab.transact();
      collab.get_map_with_txn(txn, vec![ROOT]).is_some()
    };
    if is_exist {
      Ok(Document::get_document_with_txn(collab))
    } else {
      let document = Document::create_document(collab, None)?;
      Ok(document)
    }
  }

  // Create a new document with the given data.
  pub fn create_with_data(collab: Collab, data: DocumentData) -> Result<Document, DocumentError> {
    let document = Document::create_document(collab, Some(data))?;
    Ok(document)
  }

  pub fn open<F>(&mut self, callback: F) -> Result<DocumentData, DocumentError>
  where
    F: Fn(&Vec<BlockEvent>, bool) + 'static,
  {
    self
      .subscription
      .subscribe(&mut self.root, move |block_events, origin| {
        let is_remote = origin.is_some();
        callback(block_events, is_remote);
      });
    self.get_document()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    self.root.with_transact_mut(f)
  }

  pub fn get_document(&self) -> Result<DocumentData, DocumentError> {
    let txn = self.inner.transact();
    let document_data = DocumentData {
      page_id: self
        .root
        .get_str_with_txn(&txn, PAGE_ID)
        .unwrap_or_default(),
      blocks: self.block_operation.get_all_blocks(),
      meta: DocumentMeta {
        children_map: self.children_operation.get_all_children(),
      },
    };
    Ok(document_data)
  }

  pub fn apply_action(&self, actions: Vec<BlockAction>) {
    self.inner.with_transact_mut(|txn| {
      for action in actions {
        let payload = action.payload;
        let mut block = payload.block;
        let block_id = &block.id.clone();
        let data = &block.data;
        let parent_id = payload.parent_id;
        let prev_id = payload.prev_id;

        // check if the block's parent_id is empty, if it is empty, assign the parent_id to the block
        if block.parent.is_empty() {
          if let Some(parent_id) = &parent_id {
            block.parent = parent_id.clone();
          }
        }

        let res = match action.action {
          BlockActionType::Insert => self.insert_block(txn, block, prev_id).map(|_| ()),
          BlockActionType::Update => self.update_block_data(txn, block_id, data.to_owned()),
          BlockActionType::Delete => self.delete_block(txn, block_id),
          BlockActionType::Move => self.move_block(txn, block_id, parent_id, prev_id),
        };
        if res.is_err() {
          return;
        }
      }
    })
  }

  pub fn get_block(&self, block_id: &str) -> Option<Block> {
    let txn = self.inner.transact();
    self.block_operation.get_block_with_txn(&txn, block_id)
  }

  pub fn insert_block(
    &self,
    txn: &mut TransactionMut,
    block: Block,
    prev_id: Option<String>,
  ) -> Result<Block, DocumentError> {
    let block = self.block_operation.create_block_with_txn(txn, &block)?;

    self.insert_block_to_parent(txn, &block, prev_id)
  }

  pub fn insert_block_to_parent(
    &self,
    txn: &mut TransactionMut,
    block: &Block,
    prev_id: Option<String>,
  ) -> Result<Block, DocumentError> {
    let parent_id = &block.parent;
    // If the parent is not found, return an error.
    if parent_id.is_empty() {
      return Err(DocumentError::ParentIsNotFound);
    }

    let parent = self.block_operation.get_block_with_txn(txn, parent_id);

    let parent_is_empty = parent.is_none();
    // If the parent is not found, return an error.
    if parent_is_empty {
      return Err(DocumentError::ParentIsNotFound);
    }

    let parent = parent.unwrap();
    let parent_children_id = &parent.children;
    let mut index = 0;
    // If the prev_id is not found, insert the block to the first.
    if let Some(prev_id) = prev_id {
      let prev_index =
        self
          .children_operation
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
      .children_operation
      .insert_child_with_txn(txn, parent_children_id, &block.id, index);
    Ok(
      self
        .block_operation
        .get_block_with_txn(txn, &block.id)
        .unwrap(),
    )
  }

  pub fn delete_block(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
  ) -> Result<(), DocumentError> {
    let block = self.block_operation.get_block_with_txn(txn, block_id);
    if block.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }

    let block = block.unwrap();

    // Delete all children.
    let children = self
      .children_operation
      .get_children_with_txn(txn, &block.children);
    children
      .iter(txn)
      .map(|child| child.to_string(txn))
      .collect::<Vec<String>>()
      .iter()
      .for_each(|child| self.delete_block(txn, child).unwrap_or_default());

    let parent_id = &block.parent;
    self.delete_block_from_parent(txn, block_id, parent_id);

    let res = self.block_operation.delete_block_with_txn(txn, block_id);
    match res {
      Ok(_) => Ok(()),
      Err(_) => Err(DocumentError::DeleteBlockError),
    }
  }

  pub fn delete_block_from_parent(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    parent_id: &str,
  ) {
    let parent = self.block_operation.get_block_with_txn(txn, parent_id);

    if let Some(parent) = parent {
      let parent_children_id = &parent.children;
      self
        .children_operation
        .delete_child_with_txn(txn, parent_children_id, block_id);
    }
  }

  pub fn update_block_data(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    data: HashMap<String, Value>,
  ) -> Result<(), DocumentError> {
    let block = self.block_operation.get_block_with_txn(txn, block_id);
    if block.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }
    let block = block.unwrap();
    self
      .block_operation
      .set_block_with_txn(txn, &block.id, Some(data), None)
  }

  pub fn move_block(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    parent_id: Option<String>,
    prev_id: Option<String>,
  ) -> Result<(), DocumentError> {
    let block = self.block_operation.get_block_with_txn(txn, block_id);
    if block.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }
    let block = block.unwrap();

    // If the parent is not found, return an error.
    if parent_id.is_none() {
      return Err(DocumentError::ParentIsNotFound);
    }
    let parent_id = parent_id.unwrap();
    let parent = self.block_operation.get_block_with_txn(txn, &parent_id);
    if parent.is_none() {
      return Err(DocumentError::ParentIsNotFound);
    }

    let new_parent_children_id = parent.unwrap().children;

    // If old parent is not found, return an error.
    let old_parent = self.block_operation.get_block_with_txn(txn, &block.parent);
    if old_parent.is_none() {
      return Err(DocumentError::ParentIsNotFound);
    }

    let old_parent_children_id = old_parent.unwrap().children;

    // If the prev_id is not found, insert the block to the first.
    let mut prev_index: Option<u32> = None;
    if let Some(prev_id) = prev_id {
      prev_index =
        self
          .children_operation
          .get_child_index_with_txn(txn, &new_parent_children_id, &prev_id);
    }

    let new_index = match prev_index {
      Some(prev_index) => prev_index + 1,
      None => 0,
    };

    // Delete the block from the old parent.
    self
      .children_operation
      .delete_child_with_txn(txn, &old_parent_children_id, block_id);

    // Insert the block to the new parent.
    self.children_operation.insert_child_with_txn(
      txn,
      &new_parent_children_id,
      block_id,
      new_index,
    );

    self
      .block_operation
      .set_block_with_txn(txn, block_id, Some(block.data), Some(&parent_id))
  }

  fn create_document(collab: Collab, data: Option<DocumentData>) -> Result<Self, DocumentError> {
    let res = collab.with_transact_mut(|txn| {
      // { document: {:} }
      let root = collab.create_map_with_txn(txn, ROOT);
      // { document: { blocks: {:} } }
      let blocks = root.insert_map_with_txn(txn, BLOCKS);
      // { document: { blocks: {:}, meta: {:} } }
      let meta = root.insert_map_with_txn(txn, META);
      // {document: { blocks: {:}, meta: { children_map: {:} } }
      let children_map = meta.insert_map_with_txn(txn, CHILDREN_MAP);

      let children_operation = Rc::new(ChildrenOperation::new(children_map));

      let block_operation = BlockOperation::new(blocks, Rc::clone(&children_operation));

      // If the data is not None, insert the data to the document.
      if let Some(data) = data {
        let page_id = data.page_id;
        root.insert_with_txn(txn, PAGE_ID, page_id);
        for (_id, block) in data.blocks {
          let res = block_operation.create_block_with_txn(txn, &block);
          if res.is_err() {
            return Err(res.err().unwrap());
          }
        }
        for (id, child_ids) in data.meta.children_map {
          let map = children_operation.get_children_with_txn(txn, &id);
          child_ids.iter().for_each(|child_id| {
            map.push_back(txn, child_id.to_string());
          });
        }
      };

      Ok((root, block_operation, children_operation))
    });

    if res.is_err() {
      return Err(res.err().unwrap());
    }

    let (root, block_operation, children_operation) = res.unwrap();
    let subscription = RootDeepSubscription::default();

    let document = Self {
      inner: collab,
      root,
      block_operation,
      children_operation,
      subscription,
    };

    Ok(document)
  }

  fn get_document_with_txn(collab: Collab) -> Self {
    let txn = collab.transact();

    let root = collab.get_map_with_txn(&txn, vec![ROOT]).unwrap();

    let blocks = collab.get_map_with_txn(&txn, vec![ROOT, BLOCKS]).unwrap();

    let children_map = collab
      .get_map_with_txn(&txn, vec![ROOT, META, CHILDREN_MAP])
      .unwrap();

    let children_operation = Rc::new(ChildrenOperation::new(children_map));

    let block_operation = BlockOperation::new(blocks, Rc::clone(&children_operation));

    let subscription = RootDeepSubscription::default();

    drop(txn);
    Self {
      inner: collab,
      root,
      block_operation,
      children_operation,
      subscription,
    }
  }
}
