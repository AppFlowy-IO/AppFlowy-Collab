use std::collections::HashMap;
use std::rc::Rc;

use crate::blocks::{Block, BlockOperation, ChildrenOperation, TextOperation, EXTERNAL_TYPE_TEXT};
use crate::error::DocumentError;
use collab::preclude::*;
use nanoid::nanoid;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::Value;

const ROOT: &str = "document";
const PAGE_ID: &str = "page_id";
const BLOCKS: &str = "blocks";
const META: &str = "meta";
const TEXT_MAP: &str = "text_map";
const CHILDREN_MAP: &str = "children_map";

pub struct Document {
  #[allow(dead_code)]
  inner: Collab,
  pub root: MapRefWrapper,
  text_operation: Rc<TextOperation>,
  children_operation: Rc<ChildrenOperation>,
  block_operation: BlockOperation,
}

impl Serialize for Document {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let mut s = serializer.serialize_struct("Document", 3)?;
    s.serialize_field(
      PAGE_ID,
      &self
        .root
        .get(&txn, PAGE_ID)
        .unwrap_or_else(|| YrsValue::from(""))
        .to_string(&txn),
    )?;
    let blocks = serde_json::to_value(&self.block_operation).unwrap_or_default();
    s.serialize_field(BLOCKS, &blocks)?;
    s.serialize_field(
      META,
      &serde_json::json!({
          TEXT_MAP: serde_json::to_value(self.text_operation.as_ref()).unwrap_or_default(),
          CHILDREN_MAP: serde_json::to_value(self.children_operation.as_ref()).unwrap_or_default(),
      }),
    )?;
    s.end()
  }
}

pub struct InsertBlockArgs {
  pub parent_id: String,
  pub block_id: String,
  pub data: HashMap<String, Value>,
  pub children_id: String,
  pub ty: String,
  pub external_id: Option<String>,
  pub external_type: Option<String>,
}

impl Document {
  pub fn create(collab: Collab) -> Result<Document, DocumentError> {
    let (root, blocks, text_map, children_map) = collab.with_transact_mut(|txn| {
      // { document: {:} }
      let root = collab
        .get_map_with_txn(txn, vec![ROOT])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, ROOT));
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

      (root, blocks, text_map, children_map)
    });
    let mut root = root;
    let mut blocks = blocks;
    subscribe_changes(&mut root);

    let text_operation = Rc::new(TextOperation::new(text_map));
    let children_operation = Rc::new(ChildrenOperation::new(children_map));

    subscribe_changes(&mut blocks);
    let block_operation = BlockOperation::new(
      blocks,
      Rc::clone(&children_operation),
      Rc::clone(&text_operation),
    );
    let document = Self {
      inner: collab,
      root,
      block_operation,
      text_operation,
      children_operation,
    };
    match document.initial() {
      Ok(_) => Ok(document),
      Err(err) => Err(err),
    }
  }

  pub fn initial(&self) -> Result<(), DocumentError> {
    self.inner.with_transact_mut(|txn| {
      // { document: { page_id: "xxxx" } }
      let page_id = self.root.get_str_with_txn(txn, PAGE_ID).unwrap_or_else(|| {
        let page_id = nanoid!(10);
        self.root.insert_with_txn(txn, PAGE_ID, page_id.to_string());
        page_id
      });

      // { document: { page_id: "xxxx", blocks: { xxxx: {:} } } }
      let page_block = self.block_operation.get_block_with_txn(txn, &page_id);
      let page_block = match page_block {
        Some(block) => Some(block),
        None => {
          let root_text_id = nanoid!(10);
          let root_children_id = nanoid!(10);
          let block = self.block_operation.create_block_with_txn(
            txn,
            &Block {
              id: page_id.clone(),
              parent: "".to_string(),
              children: root_children_id,
              data: HashMap::new(),
              ty: "page".to_string(),
              external_id: Some(root_text_id),
              external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
            },
          );
          match block {
            Ok(block) => Some(block),
            Err(_) => None,
          }
        },
      };

      if page_block.is_none() {
        return Err(DocumentError::CreateRootBlockError);
      }

      // { document: { page_id: "xxxx", blocks: { xxxx: {:}, first_line_id: {:} } } }
      let page_children = self
        .children_operation
        .get_children_with_txn(txn, &page_block.unwrap().children);
      if page_children.as_ref().len() > 0 {
        return Ok(());
      }
      let first_line_id = page_children.get_with_txn(txn, 0);
      if first_line_id.is_none() {
        let first_line_id = nanoid!(10);
        let first_line_text_id = nanoid!(10);
        let first_line_children_id = nanoid!(10);
        let block = self.insert_block(
          txn,
          InsertBlockArgs {
            parent_id: page_id,
            block_id: first_line_id,
            data: HashMap::new(),
            children_id: first_line_children_id,
            ty: "text".to_string(),
            external_id: Some(first_line_text_id),
            external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
          },
          "".to_string(),
        );
        return match block {
          Ok(_) => Ok(()),
          Err(_) => Err(DocumentError::BlockCreateError),
        };
      }

      Ok(())
    })
  }

  pub fn get_document(&self) -> Result<serde_json::value::Value, DocumentError> {
    let document_data = serde_json::json!({
        "document": serde_json::to_value(self).unwrap()
    });

    Ok(document_data)
  }

  pub fn get_block(&self, block_id: &str) -> Option<Block> {
    let txn = self.inner.transact();
    self.block_operation.get_block_with_txn(&txn, block_id)
  }

  pub fn insert_block(
    &self,
    txn: &mut TransactionMut,
    args: InsertBlockArgs,
    prev_id: String,
  ) -> Result<Block, DocumentError> {
    let block_id = args.block_id;
    let parent_id = args.parent_id;

    let block = self.block_operation.create_block_with_txn(
      txn,
      &Block {
        id: block_id,
        parent: parent_id,
        children: args.children_id,
        data: args.data,
        ty: args.ty,
        external_id: args.external_id,
        external_type: args.external_type,
      },
    );

    match block {
      Ok(block) => self.insert_block_to_parent(txn, &block, prev_id),
      _ => Err(DocumentError::BlockCreateError),
    }
  }

  pub fn insert_block_to_parent(
    &self,
    txn: &mut TransactionMut,
    block: &Block,
    prev_id: String,
  ) -> Result<Block, DocumentError> {
    let parent_id = &block.parent;
    if parent_id.is_empty() {
      return Err(DocumentError::ParentIsNotFound);
    }
    let parent = self.block_operation.get_block_with_txn(txn, parent_id);

    let parent_is_empty = parent.is_none();
    if parent_is_empty {
      return Err(DocumentError::ParentIsNotFound);
    }

    let parent = parent.unwrap();
    let parent_children_id = &parent.children;
    let mut index = 0;
    if !prev_id.is_empty() {
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
  ) -> Result<Block, DocumentError> {
    let block = self.block_operation.get_block_with_txn(txn, block_id);
    if block.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }

    let block = block.unwrap();

    let children = self
      .children_operation
      .get_children_with_txn(txn, &block.children);
    children
      .iter(txn)
      .map(|child| child.to_string(txn))
      .collect::<Vec<String>>()
      .iter()
      .for_each(|child| match self.delete_block(txn, child) {
        Ok(_) => (),
        Err(_) => {
          println!("delete block error");
        },
      });

    let parent_id = &block.parent;
    self.delete_block_from_parent(txn, block_id, parent_id);

    self.block_operation.delete_block_with_txn(txn, block_id)
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
    parent_id: &str,
    prev_id: &str,
  ) -> Result<(), DocumentError> {
    let block = self.block_operation.get_block_with_txn(txn, block_id);
    if block.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }
    let block = block.unwrap();

    let parent = self.block_operation.get_block_with_txn(txn, parent_id);
    if parent.is_none() {
      return Err(DocumentError::ParentIsNotFound);
    }

    let new_parent_children_id = parent.unwrap().children;

    let old_parent = self.block_operation.get_block_with_txn(txn, &block.parent);
    if old_parent.is_none() {
      return Err(DocumentError::ParentIsNotFound);
    }

    let old_parent_children_id = old_parent.unwrap().children;

    let prev_index =
      self
        .children_operation
        .get_child_index_with_txn(txn, &new_parent_children_id, prev_id);

    let new_index = match prev_index {
      Some(prev_index) => prev_index + 1,
      None => 0,
    };

    self
      .children_operation
      .delete_child_with_txn(txn, &old_parent_children_id, block_id);
    self.children_operation.insert_child_with_txn(
      txn,
      &new_parent_children_id,
      block_id,
      new_index,
    );

    self
      .block_operation
      .set_block_with_txn(txn, block_id, Some(block.data), Some(parent_id))
  }
}

fn subscribe_changes(root: &mut MapRefWrapper) -> Option<DeepEventsSubscription> {
  return Some(root.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Text(_) => {},
        Event::Array(_) => {},
        Event::Map(event) => {
          for c in event.keys(txn).values() {
            match c {
              EntryChange::Inserted(_v) => {
                println!("insert: {}", event.target().to_json(txn));
              },
              EntryChange::Updated(_k, _v) => {
                println!("update: {}", event.target().to_json(txn));
              },
              EntryChange::Removed(_v) => {
                println!("remove: {}", event.target().to_json(txn));
              },
            }
          }
        },
        _ => {},
      }
    }
  }));
}
