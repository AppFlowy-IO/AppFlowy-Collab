use crate::user_test::script_test::script::{create_database, database_test, DatabaseScript};
use collab_database::block::CreateRowParams;
use collab_database::views::CreateDatabaseParams;

#[tokio::test]
async fn create_row_test() {
  let mut test = database_test();
  let mut scripts = vec![];
  for i in 0..10 {
    scripts.push(DatabaseScript::CreateDatabase {
      params: create_database(&format!("d{}", i)),
    })
  }
  test.run_scripts(scripts).await;
}
