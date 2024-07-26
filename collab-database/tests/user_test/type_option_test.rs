use collab_database::fields::{Field, TypeOptionDataBuilder, TypeOptions};
use collab_database::views::{CreateDatabaseParams, CreateViewParams, OrderObjectPosition};

use crate::database_test::helper::default_field_settings_by_layout;
use crate::user_test::helper::{workspace_database_test, WorkspaceDatabaseTest};

#[tokio::test]
async fn update_single_type_option_data_test() {
  let test = user_database_with_default_field().await;
  let database = test.get_database("d1").await.unwrap();
  let db = database.lock().await;
  let mut collab = db.get_collab().lock().await;
  let mut txn = collab.context.transact_mut();
  db.fields.update_field(&mut txn, "f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert(
        "0",
        TypeOptionDataBuilder::new()
          .insert_str_value("task", "write code")
          .build(),
      );
    });
  });

  let field = db.fields.get_field(&txn, "f1").unwrap();
  let type_option = field.type_options.get("0").unwrap();
  assert_eq!(type_option.get("task").unwrap().to_string(), "write code");
}

#[tokio::test]
async fn insert_multi_type_options_test() {
  let test = user_database_with_default_field().await;
  let database = test.get_database("d1").await.unwrap();

  let mut type_options = TypeOptions::new();
  type_options.insert(
    "0".to_string(),
    TypeOptionDataBuilder::new()
      .insert_i64_value("job 1", 123)
      .build(),
  );
  type_options.insert(
    "1".to_string(),
    TypeOptionDataBuilder::new()
      .insert_f64_value("job 2", 456.0)
      .build(),
  );

  let db = database.lock().await;
  db.create_field(
    None,
    Field {
      id: "f2".to_string(),
      name: "second field".to_string(),
      field_type: 0,
      type_options,
      ..Default::default()
    },
    &OrderObjectPosition::default(),
    default_field_settings_by_layout(),
  );

  let collab = db.get_collab().lock().await;
  let txn = collab.context.transact();
  let second_field = db.fields.get_field(&txn, "f2").unwrap();
  assert_eq!(second_field.type_options.len(), 2);

  let type_option = second_field.type_options.get("0").unwrap();
  assert_eq!(type_option.get_i64_value("job 1").unwrap(), 123);

  let type_option = second_field.type_options.get("1").unwrap();
  assert_eq!(type_option.get_f64_value("job 2").unwrap(), 456.0);
}

async fn user_database_with_default_field() -> WorkspaceDatabaseTest {
  let test = workspace_database_test(1).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  let field = Field {
    id: "f1".to_string(),
    name: "first field".to_string(),
    field_type: 0,
    ..Default::default()
  };
  let db = database.lock().await;
  let mut collab = db.get_collab().lock().await;
  let mut txn = collab.transact_mut();
  db.fields.insert_field(&mut txn, field);
  test
}
