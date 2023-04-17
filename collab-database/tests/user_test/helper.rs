use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;

use crate::helper::TestTextCell;

use collab_database::block::CreateRowParams;
use collab_database::database::{gen_database_id, gen_field_id, gen_row_id};
use collab_database::fields::Field;
use collab_database::rows::CellsBuilder;
use collab_database::user::{RowRelationChange, RowRelationUpdateReceiver, UserDatabase};
use collab_database::views::{CreateDatabaseParams, DatabaseLayout};
use collab_persistence::CollabKV;
use rand::Rng;
use tempfile::TempDir;
use tokio::sync::mpsc::{channel, Receiver};
use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

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

pub fn random_uid() -> i64 {
  let mut rng = rand::thread_rng();
  rng.gen::<i64>()
}

pub fn user_database_test(uid: i64) -> UserDatabaseTest {
  let db = make_kv_db();
  user_database_test_with_db(uid, db)
}

pub fn user_database_test_with_db(uid: i64, db: Arc<CollabKV>) -> UserDatabaseTest {
  UserDatabaseTest {
    uid,
    inner: UserDatabase::new(uid, db.clone()),
    db,
  }
}

pub fn make_kv_db() -> Arc<CollabKV> {
  static START: Once = Once::new();
  START.call_once(|| {
    std::env::set_var("RUST_LOG", "collab_persistence=trace,collab_database=trace");
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(CollabKV::open(path).unwrap())
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

pub fn make_default_grid(view_id: &str, name: &str) -> CreateDatabaseParams {
  let text_field = Field {
    id: gen_field_id(),
    name: "Name".to_string(),
    field_type: 0,
    visibility: false,
    width: 0,
    type_options: Default::default(),
    is_primary: true,
  };

  let single_select_field = Field {
    id: gen_field_id(),
    name: "Status".to_string(),
    field_type: 3,
    visibility: false,
    width: 0,
    type_options: Default::default(),
    is_primary: false,
  };

  let checkbox_field = Field {
    id: gen_field_id(),
    name: "Done".to_string(),
    field_type: 4,
    visibility: false,
    width: 0,
    type_options: Default::default(),
    is_primary: false,
  };

  CreateDatabaseParams {
    database_id: gen_database_id(),
    view_id: view_id.to_string(),
    name: name.to_string(),
    layout: DatabaseLayout::Grid,
    layout_settings: Default::default(),
    filters: vec![],
    groups: vec![],
    sorts: vec![],
    created_rows: vec![
      CreateRowParams::new(gen_row_id()),
      CreateRowParams::new(gen_row_id()),
      CreateRowParams::new(gen_row_id()),
    ],
    fields: vec![text_field, single_select_field, checkbox_field],
  }
}
