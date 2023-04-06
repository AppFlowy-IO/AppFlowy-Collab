use collab_database::user::{RowRelationChange, RowRelationUpdateReceiver, UserDatabase};
use collab_persistence::CollabKV;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc::{channel, Receiver};

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
