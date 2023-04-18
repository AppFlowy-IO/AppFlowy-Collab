#![allow(clippy::all)]
#![allow(unused_attributes)]
#![allow(dead_code)]
#![allow(unused_assignments)]
use crate::util::db;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::{Collab, CollabBuilder};
use collab_document::blocks::DocumentData;
use collab_document::document::Document;
use collab_persistence::CollabKV;
use serde_json::Value;
use std::sync::Arc;

pub enum DocumentScript {
  CreateDocument { doc_id: String, data: DocumentData },
  AssertDocument { doc_id: String, expected: Value },
}

pub struct DocumentTest {
  user_id: i64,
  pub db: Arc<CollabKV>,
}

impl DocumentTest {
  pub fn new(uid: i64) -> Self {
    let db = db();
    Self { user_id: uid, db }
  }

  pub fn run(&self, script: DocumentScript) {
    match script {
      DocumentScript::CreateDocument { doc_id, data } => {
        let collab = get_collab_for_doc_id(&doc_id, self.user_id, self.db.clone());
        let _ = Arc::new(Document::create_with_data(collab, data).unwrap());
      },
      DocumentScript::AssertDocument { doc_id, expected } => {
        let collab = get_collab_for_doc_id(&doc_id, self.user_id, self.db.clone());
        let document = Document::create(collab).unwrap();
        let data = document.get_document().unwrap();
        let actual = serde_json::to_value(data).unwrap();
        assert_json_diff::assert_json_eq!(actual, expected);
      },
    }
  }
}

fn get_collab_for_doc_id(doc_id: &str, uid: i64, db: Arc<CollabKV>) -> Collab {
  let mut collab = CollabBuilder::new(uid, doc_id).build();
  let disk_plugin = Arc::new(CollabDiskPlugin::new(uid, db).unwrap());
  collab.add_plugin(disk_plugin);
  collab.initial();
  collab
}
