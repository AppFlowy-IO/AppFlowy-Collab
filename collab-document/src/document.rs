use arc_swap::ArcSwapOption;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use collab::core::collab::DataSource;
use collab::core::collab_state::SyncState;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::block::ClientID;
use collab::preclude::*;
use collab_entity::define::DOCUMENT_ROOT;
use collab_entity::CollabType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tokio_stream::wrappers::WatchStream;

use crate::blocks::{
  deserialize_text_delta, parse_event, Block, BlockAction, BlockActionPayload, BlockActionType,
  BlockEvent, BlockOperation, ChildrenOperation, DocumentData, DocumentMeta, TextOperation,
  EXTERNAL_TYPE_TEXT,
};
use crate::document_awareness::DocumentAwarenessState;
use crate::error::DocumentError;

/// The page_id is a reference that points to the block’s id.
/// The block that is referenced by this page_id is the first block of the document.
/// Crossing this block, we can build the whole document tree.
const PAGE_ID: &str = "page_id";
/// Document's all [Block] Map.
const BLOCKS: &str = "blocks";
/// Document's meta data.
const META: &str = "meta";
/// [Block]'s relation map. And it's also in [META].
/// The key is the parent block's children_id, and the value is the children block's id.
const CHILDREN_MAP: &str = "children_map";
/// [Block]'s yText map. And it's also in [META].
/// The key is the text block's external_id, and the value is the text block's yText.
const TEXT_MAP: &str = "text_map";

pub struct Document {
  inner: Arc<RwLock<Collab>>,
  root: MapRef,
  subscription: Option<Subscription>,
  children_operation: ChildrenOperation,
  block_operation: BlockOperation,
  text_operation: TextOperation,
  #[allow(dead_code)]
  awareness_subscription: ArcSwapOption<Subscription>,
}

impl Document {
  /// Create or get a document.
  pub async fn open<C: Into<Arc<RwLock<Collab>>>>(collab: C) -> Result<Document, DocumentError> {
    Document::open_document_with_collab(collab.into()).await
  }

  pub fn validate(collab: &Collab) -> Result<(), DocumentError> {
    CollabType::Document
      .validate_require_data(collab)
      .map_err(|_| DocumentError::NoRequiredData)?;
    Ok(())
  }

  pub fn get_collab(&self) -> &Arc<RwLock<Collab>> {
    &self.inner
  }

  pub async fn encode_collab(&self) -> Result<EncodedCollab, DocumentError> {
    let lock = self.inner.read().await;
    lock.encode_collab_v1(|collab| {
      CollabType::Document
        .validate_require_data(collab)
        .map_err(|_| DocumentError::NoRequiredData)
    })
  }

  pub fn encode_collab_blocking(&self) -> Result<EncodedCollab, DocumentError> {
    let lock = self.inner.blocking_read();
    lock.encode_collab_v1(|collab| {
      CollabType::Document
        .validate_require_data(collab)
        .map_err(|_| DocumentError::NoRequiredData)
    })
  }

  pub fn flush(&self) -> Result<(), DocumentError> {
    let collab_guard = self.inner.blocking_write();
    collab_guard.flush();
    Ok(())
  }

  /// Create a new document with the given data.
  pub async fn create_with_data(
    collab: Arc<RwLock<Collab>>,
    data: DocumentData,
  ) -> Result<Document, DocumentError> {
    Document::create_document(collab, Some(data)).await
  }

  pub async fn from_doc_state(
    origin: CollabOrigin,
    doc_state: DataSource,
    document_id: &str,
    plugins: Vec<Box<dyn CollabPlugin>>,
  ) -> Result<Self, DocumentError> {
    let collab = Collab::new_with_source(origin, document_id, doc_state, plugins, true)?;
    Document::open(Arc::new(RwLock::new(collab))).await
  }

  /// open a document and subscribe to the document changes.
  pub fn subscribe_block_changed<F>(&mut self, callback: F)
  where
    F: Fn(&Vec<BlockEvent>, bool) + Send + Sync + 'static,
  {
    let lock = self.inner.try_read().unwrap();
    let object_id = lock.object_id().to_string();
    let self_origin = lock.origin().clone();
    self.subscription = Some(self.root.observe_deep(move |txn, events| {
      let origin = CollabOrigin::from(txn);
      let block_events = events
        .iter()
        .map(|deep_event| parse_event(&object_id, txn, deep_event))
        .collect::<Vec<BlockEvent>>();
      let is_remote = self_origin != origin;
      callback(&block_events, is_remote);
    }));
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.blocking_read().subscribe_sync_state()
  }

  /// Get document data.
  pub async fn get_document_data(&self) -> Result<DocumentData, DocumentError> {
    let collab_guard = self.inner.read().await;
    let txn = collab_guard.transact();
    let page_id = self
      .root
      .get(&txn, PAGE_ID)
      .and_then(|v| v.cast::<String>().ok())
      .ok_or(DocumentError::PageIdIsEmpty)?;

    let blocks = self.block_operation.get_all_blocks(&txn);
    let children_map = self.children_operation.get_all_children(&txn);
    let text_map = self.text_operation.serialize_all_text_delta(&txn);
    let document_data = DocumentData {
      page_id,
      blocks,
      meta: DocumentMeta {
        children_map,
        text_map: Some(text_map),
      },
    };
    Ok(document_data)
  }

  /// Create a yText for incremental synchronization.
  /// - @param text_id: The text block's external_id.
  /// - @param delta: The text block's delta. "\[{"insert": "Hello", "attributes": { "bold": true, "italic": true } }, {"insert": " World!"}]".
  pub fn create_text(&self, text_id: &str, delta: String) {
    let mut lock = self.inner.blocking_write();
    let mut txn = lock.transact_mut();
    self.create_text_with_txn(&mut txn, text_id, delta);
  }

  pub fn create_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str, delta: String) {
    let delta = deserialize_text_delta(&delta).ok();
    self.text_operation.get_text_with_txn(txn, text_id);
    if let Some(delta) = delta {
      self
        .text_operation
        .apply_delta_with_txn(txn, text_id, delta);
    }
  }

  /// Apply a delta to the yText.
  /// - @param text_id: The text block's external_id.
  /// - @param delta: The text block's delta. "\[{"insert": "Hello", "attributes": { "bold": true, "italic": true } }, {"insert": " World!"}]".
  pub fn apply_text_delta(&self, text_id: &str, delta: String) {
    let mut lock = self.inner.blocking_write();
    let mut txn = lock.transact_mut();
    self.apply_text_delta_with_txn(&mut txn, text_id, delta);
  }

  pub fn apply_text_delta_with_txn(&self, txn: &mut TransactionMut, text_id: &str, delta: String) {
    let delta = deserialize_text_delta(&delta).ok();
    if let Some(delta) = delta {
      self
        .text_operation
        .apply_delta_with_txn(txn, text_id, delta);
    } else {
      self
        .text_operation
        .apply_delta_with_txn(txn, text_id, vec![]);
    }
  }

  /// Apply actions to the document.
  pub fn apply_action(&self, actions: Vec<BlockAction>) {
    let mut lock = self.inner.blocking_write();
    let mut txn = lock.transact_mut();
    for action in actions {
      let result = match action.action {
        BlockActionType::Insert => self.handle_insert_action(&mut txn, action.payload),
        BlockActionType::Update => self.handle_update_action(&mut txn, action.payload),
        BlockActionType::Delete => self.handle_delete_action(&mut txn, action.payload),
        BlockActionType::Move => self.handle_move_action(&mut txn, action.payload),
        BlockActionType::InsertText => self.handle_insert_text_action(&mut txn, action.payload),
        BlockActionType::ApplyTextDelta => {
          self.handle_apply_text_delta_action(&mut txn, action.payload)
        },
      };

      if let Err(err) = result {
        // Handle the error
        tracing::error!("[Document] apply_action error: {:?}", err);
        return;
      }
    }
  }

  /// Get block with the given id.
  pub fn get_block(&self, block_id: &str) -> Option<Block> {
    let collab_guard = self.inner.blocking_read();
    let txn = collab_guard.transact();
    self.block_operation.get_block_with_txn(&txn, block_id)
  }

  /// Insert block to the document.
  pub fn insert_block(
    &self,
    txn: &mut TransactionMut,
    block: Block,
    prev_id: Option<String>,
  ) -> Result<Block, DocumentError> {
    let block = self.block_operation.create_block_with_txn(txn, block)?;
    self.insert_block_to_parent(txn, &block, prev_id)
  }

  /// Insert block with the given parent id and prev id.
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

    let parent = match self.block_operation.get_block_with_txn(txn, parent_id) {
      Some(parent) => parent,
      None => return Err(DocumentError::ParentIsNotFound),
    };

    let parent_children_id = &parent.children;
    // If the prev_id is not found, insert the block to the first position.
    // so the default index is 0.
    let index = prev_id
      .and_then(|prev_id| {
        self
          .children_operation
          .get_child_index_with_txn(txn, parent_children_id, &prev_id)
      })
      .map(|prev_index| prev_index + 1)
      .unwrap_or(0);
    self
      .children_operation
      .insert_child_with_txn(txn, parent_children_id, &block.id, index);
    let block = self
      .block_operation
      .get_block_with_txn(txn, &block.id)
      .unwrap();
    Ok(block)
  }

  /// delete the block from the document
  ///
  /// 1. delete all the children of this block
  /// 2. delete the block from its parent
  /// 3. delete the block from the block map
  pub fn delete_block(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
  ) -> Result<(), DocumentError> {
    let block = match self.block_operation.get_block_with_txn(txn, block_id) {
      Some(block) => block,
      None => return Err(DocumentError::BlockIsNotFound),
    };

    let external_id = &block.external_id;

    // Delete all the children of this block.
    let children = self
      .children_operation
      .get_children_with_txn(txn, &block.children);
    children
      .iter(txn)
      .map(|child| child.to_string(txn))
      .collect::<Vec<String>>()
      .iter()
      .for_each(|child| self.delete_block(txn, child).unwrap_or_default());

    // Delete the block from its parent.
    let parent_id = &block.parent;
    self.delete_block_from_parent(txn, block_id, parent_id);

    // Delete the text
    if let Some(external_id) = external_id {
      self.text_operation.delete_text_with_txn(txn, external_id);
    }
    // Delete the block
    self
      .block_operation
      .delete_block_with_txn(txn, block_id)
      .map(|_| ())
  }

  /// remove the reference of the block from its parent.
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

  /// update the block data.
  pub fn update_block_data(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    data: HashMap<String, Value>,
  ) -> Result<(), DocumentError> {
    let block = match self.block_operation.get_block_with_txn(txn, block_id) {
      Some(block) => block,
      None => return Err(DocumentError::BlockIsNotFound),
    };
    self.block_operation.set_block_with_txn(
      txn,
      &block.id,
      Some(data),
      None,
      block.external_id,
      block.external_type,
    )
  }

  /// move the block to the new parent.
  pub fn move_block(
    &self,
    txn: &mut TransactionMut,
    block_id: &str,
    parent_id: Option<String>,
    prev_id: Option<String>,
  ) -> Result<(), DocumentError> {
    // If the parent is not found, return an error.
    let new_parent = match parent_id {
      Some(parent_id) => match self.block_operation.get_block_with_txn(txn, &parent_id) {
        Some(parent) => parent,
        None => return Err(DocumentError::ParentIsNotFound),
      },
      None => return Err(DocumentError::ParentIsNotFound),
    };

    let block = match self.block_operation.get_block_with_txn(txn, block_id) {
      Some(block) => block,
      None => return Err(DocumentError::BlockIsNotFound),
    };

    // If the old parent is not found, return an error.
    let old_parent = match self.block_operation.get_block_with_txn(txn, &block.parent) {
      Some(parent) => parent,
      None => return Err(DocumentError::ParentIsNotFound),
    };

    let new_parent_children_id = new_parent.children;
    let old_parent_children_id = old_parent.children;

    // It must delete the block from the old parent.
    // And this operation must be done before insert the block to the new parent,
    // because the block may be moved to the same parent.
    self
      .children_operation
      .delete_child_with_txn(txn, &old_parent_children_id, block_id);

    // If the prev_id is not found, insert the block to the first position.
    // so the default index is 0.
    let index = prev_id
      .and_then(|prev_id| {
        self
          .children_operation
          .get_child_index_with_txn(txn, &new_parent_children_id, &prev_id)
      })
      .map(|prev_index| prev_index + 1)
      .unwrap_or(0);

    // Insert the block to the new parent.
    self
      .children_operation
      .insert_child_with_txn(txn, &new_parent_children_id, block_id, index);

    // Update the parent of the block.
    self.block_operation.set_block_with_txn(
      txn,
      block_id,
      Some(block.data),
      Some(&new_parent.id),
      None,
      None,
    )
  }

  pub fn redo(&self) -> bool {
    if !self.can_redo() {
      return false;
    }
    if let Ok(mut collab_guard) = self.inner.try_write() {
      collab_guard.redo().unwrap_or_default()
    } else {
      false
    }
  }

  pub fn undo(&self) -> bool {
    if !self.can_undo() {
      return false;
    }
    if let Ok(mut collab_guard) = self.inner.try_write() {
      collab_guard.undo().unwrap_or_default()
    } else {
      false
    }
  }

  pub fn can_redo(&self) -> bool {
    if let Ok(collab_guard) = self.inner.try_read() {
      collab_guard.can_redo()
    } else {
      false
    }
  }

  pub fn can_undo(&self) -> bool {
    if let Ok(collab_guard) = self.inner.try_read() {
      collab_guard.can_undo()
    } else {
      false
    }
  }

  /// Set the local state of the awareness.
  /// It will override the previous state.
  pub fn set_awareness_local_state(&self, state: DocumentAwarenessState) {
    if let Ok(state) = serde_json::to_string(&state) {
      self
        .inner
        .blocking_write()
        .get_mut_awareness()
        .set_local_state(state);
    } else {
      tracing::error!(
        "Failed to serialize DocumentAwarenessState, state: {:?}",
        state
      );
    }
  }

  pub fn get_awareness_local_state(&self) -> Option<DocumentAwarenessState> {
    let collab = self.inner.blocking_read();
    let json = collab.get_awareness().local_state()?;
    match serde_json::from_str(json) {
      Ok(state) => Some(state),
      Err(e) => {
        tracing::error!(
          "Failed to deserialize DocumentAwarenessState, state: `{}` - {}",
          json,
          e
        );
        None
      },
    }
  }

  /// Clean the local state of the awareness.
  /// It should be called when the document is closed.
  pub fn clean_awareness_local_state(&self) {
    self
      .inner
      .blocking_write()
      .get_mut_awareness()
      .clean_local_state()
  }

  /// Subscribe to the awareness state change.
  /// This function only allowed to be called once for each document.
  pub fn subscribe_awareness_state<F>(&mut self, f: F)
  where
    F: Fn(HashMap<ClientID, DocumentAwarenessState>) + Send + Sync + 'static,
  {
    let subscription = self.inner.try_read().unwrap().get_awareness().on_update(move |awareness, e, _| {
      if let Ok(full_update) = awareness.update_with_clients(e.all_changes()) {
        let result: HashMap<_, _> = full_update.clients.iter().filter_map(|(&client_id, entry)| {
          match serde_json::from_str(&entry.json) {
            Ok(state) => Some((client_id, state)),
            Err(e) => {
              tracing::error!(
                "subscribe_awareness_state error: failed to parse state for id: {:?}, state: {:?} - {}",
                client_id,
                entry.json,
                e
              );
              None
            },
          }
        }).collect();
        f(result);
      }
    });
    self
      .awareness_subscription
      .store(Some(Arc::new(subscription)));
  }

  async fn create_document(
    collab: Arc<RwLock<Collab>>,
    data: Option<DocumentData>,
  ) -> Result<Self, DocumentError> {
    let mut collab_guard = collab.write().await;
    let (root, block_operation, children_operation, text_operation) = {
      let collab = &mut *collab_guard;
      let mut txn = collab.context.transact_mut();
      // { document: {:} }
      let root = collab.data.get_or_init_map(&mut txn, DOCUMENT_ROOT);
      // { document: { blocks: {:} } }
      let blocks = root.get_or_init_map(&mut txn, BLOCKS);
      // { document: { blocks: {:}, meta: {:} } }
      let meta = root.get_or_init_map(&mut txn, META);
      // {document: { blocks: {:}, meta: { children_map: {:} } }
      let children_map = meta.get_or_init_map(&mut txn, CHILDREN_MAP);
      // { document: { blocks: {:}, meta: { text_map: {:} } }
      let text_map = meta.get_or_init_map(&mut txn, TEXT_MAP);

      let children_operation = ChildrenOperation::new(children_map);
      let text_operation = TextOperation::new(text_map);
      let block_operation = BlockOperation::new(blocks, children_operation.clone());

      // If the data is not None, insert the data to the document.
      if let Some(data) = data {
        root.insert(&mut txn, PAGE_ID, data.page_id);

        for (_, block) in data.blocks {
          block_operation.create_block_with_txn(&mut txn, block)?;
        }

        for (id, child_ids) in data.meta.children_map {
          let map = children_operation.get_children_with_txn(&mut txn, &id);
          child_ids.iter().for_each(|child_id| {
            map.push_back(&mut txn, child_id.to_string());
          });
        }
        if let Some(text_map) = data.meta.text_map {
          for (id, delta) in text_map {
            let delta = serde_json::from_str(&delta).unwrap_or_else(|_| vec![]);
            text_operation.apply_delta_with_txn(&mut txn, &id, delta)
          }
        }
      }

      (root, block_operation, children_operation, text_operation)
    };

    collab_guard.enable_undo_redo();

    drop(collab_guard);

    let document = Self {
      inner: collab,
      root,
      block_operation,
      children_operation,
      text_operation,
      subscription: None,
      awareness_subscription: Default::default(),
    };
    Ok(document)
  }

  async fn open_document_with_collab(collab: Arc<RwLock<Collab>>) -> Result<Self, DocumentError> {
    let mut collab_guard = collab.write().await;
    let (root, block_operation, children_operation, text_operation) = {
      let collab_ref = &mut *collab_guard;
      let mut txn = collab_ref.context.transact_mut();
      let root = collab_ref.data.get_with_path(&txn, [DOCUMENT_ROOT]);
      if root.is_none() {
        return Err(DocumentError::NoRequiredData);
      }
      let root: MapRef = root.unwrap();
      let blocks = root.get_or_init_map(&mut txn, BLOCKS);
      let meta = root.get_or_init_map(&mut txn, META);

      let children_map = meta.get_or_init_map(&mut txn, CHILDREN_MAP);
      let text_map = meta.get_or_init_map(&mut txn, TEXT_MAP);
      let children_operation = ChildrenOperation::new(children_map);
      let text_operation = TextOperation::new(text_map);
      let block_operation = BlockOperation::new(blocks, children_operation.clone());
      (
        Some(root),
        Some(block_operation),
        Some(children_operation),
        Some(text_operation),
      )
    };

    collab_guard.enable_undo_redo();
    drop(collab_guard);

    if root.is_none() {}

    if block_operation.is_none() {
      return Err(DocumentError::BlockIsNotFound);
    }

    if children_operation.is_none() {
      return Err(DocumentError::Internal(anyhow::anyhow!(
        "Unexpected empty child map"
      )));
    }

    if text_operation.is_none() {
      return Err(DocumentError::Internal(anyhow::anyhow!(
        "Unexpected empty text map"
      )));
    }

    Ok(Self {
      inner: collab,
      root: root.unwrap(),
      block_operation: block_operation.unwrap(),
      children_operation: children_operation.unwrap(),
      text_operation: text_operation.unwrap(),
      subscription: None,
      awareness_subscription: Default::default(),
    })
  }

  fn handle_insert_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(mut block) = payload.block {
      // Check if the block's parent_id is empty, if it is empty, assign the parent_id to the block
      if block.parent.is_empty() && payload.parent_id.is_some() {
        block.parent = payload.parent_id.unwrap();
      }
      self.insert_block(txn, block, payload.prev_id).map(|_| ())
    } else {
      Err(DocumentError::BlockIsNotFound)
    }
  }

  fn handle_update_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(block) = payload.block {
      let data = &block.data;
      self.update_block_data(txn, &block.id, data.to_owned())
    } else {
      Err(DocumentError::BlockIsNotFound)
    }
  }

  fn handle_delete_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(block) = payload.block {
      self.delete_block(txn, &block.id)
    } else {
      Err(DocumentError::BlockIsNotFound)
    }
  }

  fn handle_move_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(block) = payload.block {
      self.move_block(txn, &block.id, payload.parent_id, payload.prev_id)
    } else {
      Err(DocumentError::BlockIsNotFound)
    }
  }

  fn handle_insert_text_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(text_id) = payload.text_id {
      if let Some(delta) = payload.delta {
        self.create_text_with_txn(txn, &text_id, delta);
        Ok(())
      } else {
        Err(DocumentError::TextActionParamsError)
      }
    } else {
      Err(DocumentError::TextActionParamsError)
    }
  }

  fn handle_apply_text_delta_action(
    &self,
    txn: &mut TransactionMut,
    payload: BlockActionPayload,
  ) -> Result<(), DocumentError> {
    if let Some(text_id) = payload.text_id {
      if let Some(delta) = payload.delta {
        self.apply_text_delta_with_txn(txn, &text_id, delta);
        Ok(())
      } else {
        Err(DocumentError::TextActionParamsError)
      }
    } else {
      Err(DocumentError::TextActionParamsError)
    }
  }
}

/// Represents a the index content of a document.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DocumentIndexContent {
  pub page_id: String,
  pub text: String,
}

impl From<&Document> for DocumentIndexContent {
  fn from(value: &Document) -> Self {
    let collab_guard = value.inner.blocking_read();
    let txn = collab_guard.transact();
    let page_id = value
      .root
      .get_with_txn(&txn, PAGE_ID)
      .expect("document should have page_id");

    let blocks = value.block_operation.get_all_blocks(&txn);
    let children_map = value.children_operation.get_all_children(&txn);
    let text_map = value.text_operation.stringify_all_text_delta(&txn);

    drop(txn);
    drop(collab_guard);

    let page_block = blocks
      .get(&page_id)
      .expect("document data should contain page block");
    let children_key = &page_block.children;
    let children_ids = children_map
      .get(children_key)
      .expect("children map should contain page's children key");

    let text: Vec<_> = children_ids
      .iter()
      .filter_map(|id| blocks.get(id)) // get block of child
      .filter_map(|block| { // get external id of blocks with external type text
        let Some(ty) = block.external_type.as_ref() else { return None;};
        if ty == EXTERNAL_TYPE_TEXT {
          return block.external_id.as_ref();
        }
        None
      })
      .filter_map(|ext_id| text_map.get(ext_id).filter(|t| !t.is_empty())) // get text of block
      .cloned()
      .collect();

    let text = text.join(" "); // all text of document

    Self { page_id, text }
  }
}
