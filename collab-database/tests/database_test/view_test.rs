use std::sync::Arc;

use assert_json_diff::assert_json_eq;
use nanoid::nanoid;

use collab::preclude::Any;
use collab::util::AnyMapExt;
use collab_database::database::{gen_row_id, DatabaseData};
use collab_database::entity::CreateViewParams;
use collab_database::fields::Field;
use collab_database::rows::CreateRowParams;
use collab_database::views::{DatabaseLayout, LayoutSettingBuilder, OrderObjectPosition};

use crate::database_test::helper::{
  create_database, create_database_with_default_data, default_field_settings_by_layout,
};
use crate::helper::TestFilter;

#[tokio::test]
async fn create_initial_database_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  assert_eq!(database_test.get_all_field_orders().len(), 0);
  assert_eq!(database_test.get_all_rows().await.len(), 0);
  assert_eq!(database_test.get_database_id(), database_id);

  let inline_view_id = database_test.get_inline_view_id();
  assert_eq!(inline_view_id, "v1".to_string());

  let mut views = database_test.get_all_views();
  assert_eq!(views.len(), 1);

  let inline_view = views.remove(
    views
      .iter()
      .position(|view| view.id == inline_view_id)
      .unwrap(),
  );

  assert_eq!(inline_view.database_id, database_id);
  assert_eq!(inline_view.name, "my first database view".to_string());
}

#[tokio::test]
async fn create_database_with_single_view_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.row_orders.len(), 3);
  assert_eq!(view.field_orders.len(), 3);
}

#[tokio::test]
async fn get_database_views_meta_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let views = database_test.get_all_database_views_meta();
  assert_eq!(views.len(), 1);
  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.name, "my first database view");
}

#[tokio::test]
async fn create_same_database_view_twice_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    name: "my second grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  let view = database_test.get_view("v1").unwrap();

  assert_eq!(view.name, "my second grid");
}

#[tokio::test]
async fn create_database_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;
  let row_id = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(row_id.clone(), database_id.clone()))
    .await
    .unwrap();

  let view = database_test.get_view("v1").unwrap();
  assert_json_eq!(view.row_orders.last().unwrap().id, row_id);
}

#[tokio::test]
async fn create_database_field_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;

  let field_id = nanoid!(4);
  database_test.create_field(
    None,
    Field {
      id: field_id.clone(),
      name: "my third field".to_string(),
      ..Default::default()
    },
    &OrderObjectPosition::default(),
    default_field_settings_by_layout(),
  );

  let view = database_test.get_view("v1").unwrap();
  assert_json_eq!(view.field_orders.last().unwrap().id, field_id);
}

#[tokio::test]
async fn create_database_view_with_filter_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;
  let filter_1 = TestFilter {
    id: "filter1".to_string(),
    field_id: "".to_string(),
    field_type: Default::default(),
    condition: 0,
    content: "".to_string(),
  };

  let filter_2 = TestFilter {
    id: "filter2".to_string(),
    field_id: "".to_string(),
    field_type: Default::default(),
    condition: 0,
    content: "".to_string(),
  };

  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    filters: vec![filter_1.into(), filter_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let view = database_test.get_view("v1").unwrap();
  let filters = view
    .filters
    .into_iter()
    .map(|value| TestFilter::try_from(value).unwrap())
    .collect::<Vec<TestFilter>>();
  assert_eq!(filters.len(), 2);
  assert_eq!(filters[0].id, "filter1");
  assert_eq!(filters[1].id, "filter2");
}

#[tokio::test]
async fn create_database_view_with_layout_setting_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;
  let grid_setting =
    LayoutSettingBuilder::from([("1".into(), 123.into()), ("2".into(), "abc".into())]);

  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  }
  .with_layout_setting(grid_setting);
  database_test.create_linked_view(params).unwrap();

  let view = database_test.get_view("v1").unwrap();
  let grid_layout_setting = view.layout_settings.get(&DatabaseLayout::Grid).unwrap();
  assert_eq!(grid_layout_setting.get_as::<i64>("1").unwrap(), 123);
  assert_eq!(
    grid_layout_setting.get("2").unwrap(),
    &Any::String(Arc::from("abc".to_string()))
  );
}

#[tokio::test]
async fn delete_database_view_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;
  for i in 2..5 {
    let params = CreateViewParams {
      database_id: "1".to_string(),
      view_id: format!("v{}", i),
      ..Default::default()
    };
    database_test.create_linked_view(params).unwrap();
  }

  let views = database_test.get_all_views();
  assert_eq!(views.len(), 4);

  let deleted_view_id = "v3".to_string();
  database_test.delete_view(&deleted_view_id);
  let views = database_test
    .get_all_views()
    .iter()
    .map(|view| view.id.clone())
    .collect::<Vec<String>>();
  assert_eq!(views.len(), 3);
  assert!(!views.contains(&deleted_view_id));
}

#[tokio::test]
async fn duplicate_database_view_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;

  let views = database_test.get_all_views();
  assert_eq!(views.len(), 1);

  let view = database_test.get_view("v1").unwrap();
  let duplicated_view = database_test.duplicate_linked_view("v1").unwrap();

  let views = database_test.get_all_views();
  assert_eq!(views.len(), 2);

  assert_eq!(duplicated_view.name, format!("{}-copy", view.name));
  assert_ne!(view.id, duplicated_view.id);
  // modified and created time should also be different but the test completes within one second.
}

#[tokio::test]
async fn database_data_serde_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let database_data = database_test.get_database_data().await;

  let json = database_data.to_json().unwrap();
  let database_data2 = DatabaseData::from_json(&json).unwrap();
  assert_eq!(database_data.fields.len(), database_data2.fields.len());
  assert_eq!(database_data.rows.len(), database_data2.rows.len());
}

#[tokio::test]
async fn get_database_view_layout_test() {
  let database_test = create_database_with_default_data(1, "1").await;

  let layout = database_test.get_database_view_layout("v1");
  assert_eq!(layout, DatabaseLayout::Grid);
}

#[tokio::test]
async fn update_database_view_layout_test() {
  let mut database_test = create_database_with_default_data(1, "1").await;
  database_test.update_database_view("v1", |update| {
    update.set_layout_type(DatabaseLayout::Calendar);
  });

  let layout = database_test.get_database_view_layout("v1");
  assert_eq!(layout, DatabaseLayout::Calendar);
}

#[tokio::test]
async fn validate_database_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  assert!(database_test.database.validate().is_ok())
}
