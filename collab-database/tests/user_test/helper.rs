use collab_database::user::UserDatabase;
use collab_persistence::CollabKV;
use std::ops::Deref;
use std::sync::Arc;
use tempfile::TempDir;

pub struct UserDatabaseTest {
  uid: i64,
  inner: UserDatabase,
  pub db: Arc<CollabKV>,
}

impl UserDatabaseTest {
  pub fn open_user_database(&self) -> UserDatabase {
    UserDatabase::new(self.uid, self.db.clone())
  }
}

impl Deref for UserDatabaseTest {
  type Target = UserDatabase;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub fn create_user_database(uid: i64) -> UserDatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path).unwrap());
  UserDatabaseTest {
    uid,
    inner: UserDatabase::new(uid, db.clone()),
    db,
  }
}
