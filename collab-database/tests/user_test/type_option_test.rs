use collab::util::AnyMapExt;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_database::fields::{Field, TypeOptionDataBuilder, TypeOptions};
use collab_database::views::OrderObjectPosition;
use uuid::Uuid;

use crate::database_test::helper::{
  DatabaseTest, create_database_with_params, default_field_settings_by_layout,
};

#[tokio::test]
async fn update_single_type_option_data_test() {
  let (mut database, _) = user_database_with_default_field().await;
  database.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert(
        "0",
        TypeOptionDataBuilder::from([("task".into(), "write code".into())]),
      );
    });
  });

  let field = database.get_field("f1").unwrap();
  let type_option = field.type_options.get("0").unwrap();
  assert_eq!(type_option.get("task").unwrap().to_string(), "write code");
}

#[tokio::test]
async fn insert_multi_type_options_test() {
  let (mut database, _) = user_database_with_default_field().await;

  let mut type_options = TypeOptions::new();
  type_options.insert(
    "0".to_string(),
    TypeOptionDataBuilder::from([("job 1".into(), 123.into())]),
  );
  type_options.insert(
    "1".to_string(),
    TypeOptionDataBuilder::from([("job 2".into(), (456.0).into())]),
  );

  database.create_field(
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

  let second_field = database.get_field("f2").unwrap();
  assert_eq!(second_field.type_options.len(), 2);

  let type_option = second_field.type_options.get("0").unwrap();
  assert_eq!(type_option.get_as::<i64>("job 1").unwrap(), 123);

  let type_option = second_field.type_options.get("1").unwrap();
  assert_eq!(type_option.get_as::<f64>("job 2").unwrap(), 456.0);
}

async fn user_database_with_default_field() -> (DatabaseTest, String) {
  let database_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  let field = Field {
    id: "f1".to_string(),
    name: "first field".to_string(),
    field_type: 0,
    ..Default::default()
  };
  database.insert_field(field.clone());
  (database, database_id.to_string())
}
