use crate::user_test::script_test::script::{create_database, database_test, DatabaseScript};
use collab_database::block::CreateRowParams;
use collab_database::views::CreateDatabaseParams;

#[tokio::test]
async fn create_row_test() {
  let mut test = database_test();
  let mut scripts = vec![];
  for i in 4..20 {
    let database_id = format!("d{}", i);
    scripts.push(DatabaseScript::CreateDatabase {
      params: create_database(&database_id),
    });

    scripts.push(DatabaseScript::CreateRow {
      database_id: database_id.clone(),
      params: CreateRowParams {
        id: i.into(),
        cells: Default::default(),
        height: 0,
        visibility: false,
        prev_row_id: None,
      },
    });
  }
  test.run_scripts(scripts).await;
}
