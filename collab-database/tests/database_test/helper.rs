use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use collab::preclude::CollabBuilder;
use collab_database::blocks::Block;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::Field;
use collab_database::rows::{CellsBuilder, CreateRowParams};
use collab_database::user::UserDatabaseCollabBuilder;
use collab_database::views::CreateDatabaseParams;
use collab_persistence::kv::rocks_kv::RocksCollabDB;

use tempfile::TempDir;

pub use crate::helper::*;
use crate::user_test::helper::UserDatabaseCollabBuilderImpl;

pub struct DatabaseTest {
  database: Database,

  #[allow(dead_code)]
  cleaner: Option<Cleaner>,
}

unsafe impl Send for DatabaseTest {}

unsafe impl Sync for DatabaseTest {}

impl Deref for DatabaseTest {
  type Target = Database;

  fn deref(&self) -> &Self::Target {
    &self.database
  }
}

impl DerefMut for DatabaseTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.database
  }
}

/// Create a database with a single view.
pub fn create_database(uid: i64, database_id: &str) -> DatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  let collab = CollabBuilder::new(uid, database_id).build();
  collab.initial();
  let collab_builder = Arc::new(UserDatabaseCollabBuilderImpl());
  let block = Block::new(uid, db, collab_builder.clone());
  let context = DatabaseContext {
    collab: Arc::new(collab),
    block,
    collab_builder,
  };
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    name: "my first database view".to_string(),
    ..Default::default()
  };
  let database = Database::create_with_inline_view(params, context).unwrap();
  DatabaseTest {
    database,
    cleaner: None,
  }
}

pub fn create_database_with_db(uid: i64, database_id: &str) -> (Arc<RocksCollabDB>, DatabaseTest) {
  let db = make_rocks_db();
  let collab_builder = Arc::new(UserDatabaseCollabBuilderImpl());
  let collab = collab_builder.build(uid, database_id, "database", db.clone());
  let block = Block::new(uid, db.clone(), collab_builder.clone());
  let context = DatabaseContext {
    collab,
    block,
    collab_builder,
  };
  let params = CreateDatabaseParams {
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    database_id: database_id.to_string(),
    ..Default::default()
  };
  let database = Database::create_with_inline_view(params, context).unwrap();
  (
    db,
    DatabaseTest {
      database,
      cleaner: None,
    },
  )
}

pub fn restore_database_from_db(
  uid: i64,
  database_id: &str,
  db: Arc<RocksCollabDB>,
) -> DatabaseTest {
  let collab_builder = Arc::new(UserDatabaseCollabBuilderImpl());
  let collab = collab_builder.build(uid, database_id, "database", db.clone());
  let block = Block::new(uid, db, collab_builder.clone());
  let context = DatabaseContext {
    collab,
    block,
    collab_builder,
  };
  let database = Database::get_or_create(database_id, context).unwrap();
  DatabaseTest {
    database,
    cleaner: None,
  }
}

/// Create a database with default data
/// It will create a default view with id 'v1'
pub fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = CreateRowParams {
    id: 1.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("1f1cell"))
      .insert_cell("f2", TestTextCell::from("1f2cell"))
      .insert_cell("f3", TestTextCell::from("1f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("2f1cell"))
      .insert_cell("f2", TestTextCell::from("2f2cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };
  let row_3 = CreateRowParams {
    id: 3.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("3f1cell"))
      .insert_cell("f3", TestTextCell::from("3f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };

  let database_test = create_database(uid, database_id);
  database_test.create_row(row_1);
  database_test.create_row(row_2);
  database_test.create_row(row_3);

  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  database_test.create_field(field_1);
  database_test.create_field(field_2);
  database_test.create_field(field_3);

  database_test
}

struct Cleaner(PathBuf);

impl Cleaner {
  #[allow(dead_code)]
  fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}
