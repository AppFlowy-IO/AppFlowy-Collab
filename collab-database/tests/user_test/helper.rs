use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use collab::core::any_map::AnyMapExtension;
use collab::preclude::lib0Any;
use collab_database::block::CreateRowParams;
use collab_database::fields::Field;
use collab_database::rows::{Cell, CellsBuilder};
use collab_database::user::{RowRelationChange, RowRelationUpdateReceiver, UserDatabase};
use collab_database::views::CreateDatabaseParams;
use collab_persistence::CollabKV;
use tokio::sync::mpsc::{channel, Receiver};

use tempfile::TempDir;

pub struct UserDatabaseTest {
  #[allow(dead_code)]
  uid: i64,
  inner: UserDatabase,
  pub db: Arc<CollabKV>,
}

impl Deref for UserDatabaseTest {
  type Target = UserDatabase;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub fn user_database_test(uid: i64) -> UserDatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path).unwrap());
  UserDatabaseTest {
    uid,
    inner: UserDatabase::new(uid, db.clone()),
    db,
  }
}

pub fn user_database_test_with_default_data(uid: i64) -> UserDatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path).unwrap());
  let user_database = UserDatabaseTest {
    uid,
    inner: UserDatabase::new(uid, db.clone()),
    db,
  };

  user_database
    .create_database(create_database_params("d1"))
    .unwrap();

  user_database
}

fn create_database_params(database_id: &str) -> CreateDatabaseParams {
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
  };
  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  CreateDatabaseParams {
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    name: "my first database".to_string(),
    layout: Default::default(),
    layout_settings: Default::default(),
    filters: vec![],
    groups: vec![],
    sorts: vec![],
    created_rows: vec![row_1, row_2, row_3],
    fields: vec![field_1, field_2, field_3],
  }
}

pub fn poll_row_relation_rx(mut rx: RowRelationUpdateReceiver) -> Receiver<RowRelationChange> {
  let (tx, ret) = channel(1);
  tokio::spawn(async move {
    let cloned_tx = tx.clone();
    while let Ok(change) = rx.recv().await {
      cloned_tx.send(change).await.unwrap();
    }
  });
  ret
}

pub async fn test_timeout<F: Future>(f: F) -> F::Output {
  tokio::time::timeout(Duration::from_secs(2), f)
    .await
    .unwrap()
}

pub struct TestTextCell(pub String);

impl From<TestTextCell> for Cell {
  fn from(text_cell: TestTextCell) -> Self {
    let mut cell = Self::new();
    cell.insert(
      "data".to_string(),
      lib0Any::String(text_cell.0.into_boxed_str()),
    );
    cell
  }
}

impl From<Cell> for TestTextCell {
  fn from(cell: Cell) -> Self {
    let data = cell.get_str_value("data").unwrap();
    Self(data)
  }
}

impl From<&str> for TestTextCell {
  fn from(s: &str) -> Self {
    Self(s.to_string())
  }
}
