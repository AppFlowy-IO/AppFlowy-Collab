use std::collections::HashMap;
use std::sync::Arc;

use crate::util::document_storage;

use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::Collab;
use collab_document::blocks::{
  Block, BlockAction, BlockActionPayload, BlockActionType, BlockEvent, DocumentData, DocumentMeta,
};
use collab_document::document::Document;
use collab_entity::CollabType;
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use nanoid::nanoid;
use serde_json::{Value, json};
use uuid::Uuid;

pub const TEXT_BLOCK_TYPE: &str = "paragraph";

pub struct BlockTestCore {
  pub db: Arc<CollabKVDB>,
  pub document: Document,
}

impl BlockTestCore {
  pub fn new() -> Self {
    let workspace_id = Uuid::new_v4().to_string();
    let db = document_storage();
    let doc_id = "1";
    let disk_plugin = RocksdbDiskPlugin::new(
      1,
      workspace_id.clone(),
      doc_id.to_string(),
      CollabType::Document,
      Arc::downgrade(&db),
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&db),
      uid: 1,
      workspace_id,
    };

    let options = CollabOptions::new(doc_id.to_string(), default_client_id())
      .with_data_source(data_source.into());
    let client = CollabClient::new(1, "1");
    let mut collab = Collab::new_with_options(CollabOrigin::Client(client), options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));
    collab.initialize();

    let document_data = BlockTestCore::get_default_data();
    let document = match Document::create_with_data(collab, document_data) {
      Ok(document) => document,
      Err(e) => panic!("create document error: {:?}", e),
    };
    BlockTestCore { db, document }
  }

  pub fn open(collab: Collab, db: Arc<CollabKVDB>) -> Self {
    let document = Document::open(collab).unwrap();
    BlockTestCore { db, document }
  }

  pub fn subscribe<F>(&mut self, key: &str, callback: F)
  where
    F: Fn(&Vec<BlockEvent>, bool) + Send + Sync + 'static,
  {
    self.document.subscribe_block_changed(key, callback);
  }

  pub fn get_default_data() -> DocumentData {
    let mut blocks = HashMap::new();
    let mut children_map = HashMap::new();
    let mut text_map = HashMap::new();
    let data = HashMap::new();
    let page_id = generate_id();
    let page_children_id = generate_id();
    blocks.insert(
      page_id.clone(),
      Block {
        id: page_id.clone(),
        ty: "page".to_string(),
        parent: "".to_string(),
        children: page_children_id.clone(),
        data: data.clone(),
        external_id: None,
        external_type: None,
      },
    );

    let first_text_id = generate_id();
    children_map.insert(page_children_id, vec![first_text_id.clone()]);
    let first_text_children_id = generate_id();
    children_map.insert(first_text_children_id.clone(), vec![]);
    let first_text_external_id = generate_id();
    let empty_text_delta = "[]".to_string();
    text_map.insert(first_text_external_id.clone(), empty_text_delta);
    blocks.insert(
      first_text_id.clone(),
      Block {
        id: first_text_id,
        ty: TEXT_BLOCK_TYPE.to_string(),
        parent: page_id.clone(),
        children: first_text_children_id,
        data,
        external_id: Some(first_text_external_id),
        external_type: Some("text".to_string()),
      },
    );
    let meta = DocumentMeta {
      children_map,
      text_map: Some(text_map),
    };
    DocumentData {
      page_id,
      blocks,
      meta,
    }
  }

  pub fn get_document_data(&self) -> DocumentData {
    self
      .document
      .get_document_data()
      .unwrap_or_else(|e| panic!("get document data error: {}", e))
  }

  pub fn get_page(&self) -> Block {
    let document_data = self.get_document_data();
    let page_id = document_data.page_id;
    self.get_block(&page_id)
  }

  pub fn get_block(&self, block_id: &str) -> Block {
    self
      .document
      .get_block(block_id)
      .unwrap_or_else(|| panic!("get block error: {}", block_id))
  }

  pub fn get_text_delta_with_text_id(&self, text_id: &str) -> String {
    let document_data = self.get_document_data();
    let text_map = document_data.meta.text_map.unwrap();
    text_map
      .get(text_id)
      .unwrap_or_else(|| panic!("get text delta error: {}", text_id))
      .clone()
  }

  pub fn get_block_children(&self, block_id: &str) -> Vec<Block> {
    let block = self.get_block(block_id);
    let block_children_id = block.children;
    let document_data = self.get_document_data();
    let children_map = document_data.meta.children_map;
    let children_ids = children_map
      .get(&block_children_id)
      .unwrap_or_else(|| panic!("get page children error"));
    let mut children = vec![];
    for child_id in children_ids {
      let child = self.get_block(child_id);
      children.push(child);
    }
    children
  }

  pub fn create_text(&mut self, delta: String) -> String {
    let external_id = generate_id();
    self.document.apply_text_delta(&external_id, delta);
    external_id
  }

  pub fn get_text_block(&mut self, text: String, parent_id: &str) -> Block {
    let data = HashMap::new();
    let delta = json!([{ "insert": text }]).to_string();
    let external_id = self.create_text(delta);
    Block {
      id: generate_id(),
      ty: TEXT_BLOCK_TYPE.to_string(),
      parent: parent_id.to_string(),
      children: generate_id(),
      external_id: Some(external_id),
      external_type: Some("text".to_string()),
      data,
    }
  }

  pub fn insert_text_block(
    &mut self,
    text: String,
    parent_id: &str,
    prev_id: Option<String>,
  ) -> Block {
    let block = self.get_text_block(text, parent_id);
    self
      .document
      .insert_block(block, prev_id)
      .unwrap_or_else(|e| panic!("insert block error: {:?}", e))
  }

  pub fn update_block_data(&mut self, block_id: &str, data: HashMap<String, Value>) {
    let block = self.get_block(block_id);
    self
      .document
      .update_block(block.id.as_str(), data)
      .unwrap_or_else(|e| panic!("update block error: {:?}", e));
  }

  pub fn delete_block(&mut self, block_id: &str) {
    self
      .document
      .delete_block(block_id)
      .unwrap_or_else(|e| panic!("delete block error: {:?}", e));
  }

  pub fn move_block(&mut self, block_id: &str, parent_id: &str, prev_id: Option<String>) {
    self
      .document
      .move_block(block_id, Some(parent_id.to_string()), prev_id)
      .unwrap_or_else(|e| panic!("move block error: {:?}", e));
  }

  pub fn apply_action(&mut self, actions: Vec<BlockAction>) -> bool {
    self.document.apply_action(actions).is_ok()
  }

  pub fn get_insert_action(
    &mut self,
    text: String,
    parent_id: &str,
    prev_id: Option<String>,
  ) -> BlockAction {
    let block = self.get_text_block(text, parent_id);
    BlockAction {
      action: BlockActionType::Insert,
      payload: BlockActionPayload {
        block: Some(block),
        delta: None,
        prev_id,
        parent_id: Some(parent_id.to_string()),
        text_id: None,
      },
    }
  }

  pub fn get_update_action(&self, text: String, block_id: &str) -> BlockAction {
    let block = self.get_block(block_id);
    let parent_id = block.parent.to_string();
    let mut data = HashMap::new();
    data.insert("delta".to_string(), json!([{ "insert": text }]));

    BlockAction {
      action: BlockActionType::Update,
      payload: BlockActionPayload {
        block: Some(Block { data, ..block }),
        delta: None,
        prev_id: None,
        parent_id: Some(parent_id),
        text_id: None,
      },
    }
  }

  pub fn get_delete_action(&self, block_id: &str) -> BlockAction {
    BlockAction {
      action: BlockActionType::Delete,
      payload: BlockActionPayload {
        prev_id: None,
        parent_id: None,
        block: Some(self.get_block(block_id)),
        delta: None,
        text_id: None,
      },
    }
  }

  pub fn get_move_action(
    &self,
    block_id: &str,
    parent_id: &str,
    prev_id: Option<String>,
  ) -> BlockAction {
    BlockAction {
      action: BlockActionType::Move,
      payload: BlockActionPayload {
        block: Some(self.get_block(block_id)),
        delta: None,
        prev_id,
        parent_id: Some(parent_id.to_string()),
        text_id: None,
      },
    }
  }
}

pub fn generate_id() -> String {
  nanoid!(10)
}
