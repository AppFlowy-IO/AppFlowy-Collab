use std::sync::Arc;
use uuid::Uuid;

use assert_json_diff::assert_json_eq;
use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::database::database::{DatabaseBody, DatabaseData, gen_row_id};
use collab::database::entity::{CreateDatabaseParams, CreateViewParams};
use collab::database::fields::Field;
use collab::database::rows::{CreateRowParams, Row};
use collab::database::views::{DatabaseLayout, LayoutSetting, OrderObjectPosition};
use collab::preclude::{Any, Collab};
use collab::util::AnyMapExt;
use futures::StreamExt;
use nanoid::nanoid;

use crate::database_test::helper::{
  TEST_VIEW_ID_V1, create_database, create_database_with_default_data,
  default_field_settings_by_layout,
};
use crate::helper::{SortCondition, TestFieldSetting, TestFieldType, TestFilter, TestSort};

#[tokio::test]
async fn create_initial_database_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);

  let all_rows: Vec<Row> = database_test
    .get_all_rows(20, None, false)
    .await
    .unwrap()
    .filter_map(|result| async move { result.ok() })
    .collect()
    .await;
  assert_eq!(database_test.get_all_field_orders().len(), 0);
  assert_eq!(all_rows.len(), 0);
  assert_eq!(
    database_test.get_database_id().unwrap().to_string(),
    database_id
  );

  let views = database_test.get_all_views(false);
  assert_eq!(views.len(), 1);
  assert_eq!(views[0].database_id.to_string(), database_id);
  assert_ne!(views[0].database_id.to_string(), views[0].id.to_string());
  assert_eq!(views[0].name, "my first database view".to_string());

  let encoded_collab = database_test
    .encode_collab_v1(|_| Ok::<_, anyhow::Error>(()))
    .unwrap();
  let options =
    CollabOptions::new(Uuid::new_v4(), default_client_id()).with_data_source(encoded_collab.into());
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let database_id_from_collab = DatabaseBody::database_id_from_collab(&collab).unwrap();
  assert_eq!(database_id_from_collab, database_id);
}

#[tokio::test]
async fn create_database_with_single_view_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert_eq!(view.row_orders.len(), 3);
  assert_eq!(view.field_orders.len(), 3);
}

#[tokio::test]
async fn get_database_views_meta_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let views = database_test.get_all_database_views_meta();
  assert_eq!(views.len(), 1);
  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert_eq!(view.name, "my first database view");
}

#[tokio::test]
async fn create_same_database_view_twice_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let params = CreateViewParams {
    database_id,
    view_id: uuid::Uuid::parse_str(TEST_VIEW_ID_V1).unwrap(),
    name: "my second grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();

  assert_eq!(view.name, "my second grid");
}

#[tokio::test]
async fn create_database_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;
  let row_id = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(
      row_id,
      collab::entity::uuid_validation::try_parse_database_id(&database_id).unwrap(),
    ))
    .await
    .unwrap();

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert_json_eq!(view.row_orders.last().unwrap().id, row_id);
}

#[tokio::test]
async fn create_database_field_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;

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

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert_json_eq!(view.field_orders.last().unwrap().id, field_id);
}

#[tokio::test]
async fn create_database_view_with_filter_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
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
    database_id,
    view_id: uuid::Uuid::parse_str(TEST_VIEW_ID_V1).unwrap(),
    name: "my first grid".to_string(),
    filters: vec![filter_1.into(), filter_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
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
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let grid_setting = LayoutSetting::from([("1".into(), 123.into()), ("2".into(), "abc".into())]);

  let params = CreateViewParams {
    database_id,
    view_id: uuid::Uuid::parse_str(TEST_VIEW_ID_V1).unwrap(),
    name: "my first grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  }
  .with_layout_setting(grid_setting);
  database_test.create_linked_view(params).unwrap();

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  let grid_layout_setting = view.layout_settings.get(&DatabaseLayout::Grid).unwrap();
  assert_eq!(grid_layout_setting.get_as::<i64>("1").unwrap(), 123);
  assert_eq!(
    grid_layout_setting.get("2").unwrap(),
    &Any::String(Arc::from("abc".to_string()))
  );
}

#[tokio::test]
async fn delete_database_view_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let view_ids = vec![
    uuid::Uuid::new_v4(),
    uuid::Uuid::new_v4(),
    uuid::Uuid::new_v4(),
  ];
  for view_id in &view_ids {
    let params = CreateViewParams {
      database_id,
      view_id: *view_id,
      ..Default::default()
    };
    database_test.create_linked_view(params).unwrap();
  }

  let views = database_test.get_all_views(false);
  assert_eq!(views.len(), 4);

  let deleted_view_id = view_ids[1].to_string();
  database_test.delete_view(&deleted_view_id);
  let views = database_test
    .get_all_views(false)
    .iter()
    .map(|view| view.id)
    .map(|id| id.to_string())
    .collect::<Vec<String>>();
  assert_eq!(views.len(), 3);
  assert!(!views.contains(&deleted_view_id));
}

#[tokio::test]
async fn duplicate_database_view_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;

  let views = database_test.get_all_views(false);
  assert_eq!(views.len(), 1);

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  let duplicated_view = database_test
    .duplicate_linked_view(TEST_VIEW_ID_V1)
    .unwrap();

  let views = database_test.get_all_views(false);
  assert_eq!(views.len(), 2);

  assert_eq!(duplicated_view.name, format!("{}-copy", view.name));
  assert_ne!(view.id, duplicated_view.id);
  assert_eq!(view.database_id, duplicated_view.database_id);
}

#[tokio::test]
async fn duplicate_database_view_with_custom_id_copies_settings_and_isolated_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;

  database_test.insert_filter(
    TEST_VIEW_ID_V1,
    TestFilter {
      id: "filter-1".to_string(),
      field_id: "f1".to_string(),
      field_type: TestFieldType::RichText,
      condition: 0,
      content: "contains 1".to_string(),
    },
  );
  database_test.insert_sort(
    TEST_VIEW_ID_V1,
    TestSort {
      id: "sort-1".to_string(),
      field_id: "f1".to_string(),
      field_type: i64::from(TestFieldType::RichText),
      condition: SortCondition::Descending,
    },
  );
  database_test.update_field_settings(
    TEST_VIEW_ID_V1,
    Some(vec!["f1".to_string()]),
    TestFieldSetting {
      width: 320,
      visibility: 1,
    },
  );
  database_test.update_database_view(TEST_VIEW_ID_V1, |view| {
    view.move_field_order("f3", "f1");
  });

  let source_view_before = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  let source_order_before: Vec<String> = source_view_before
    .field_orders
    .iter()
    .map(|field| field.id.clone())
    .collect();
  let source_filters_before: Vec<TestFilter> = database_test.get_all_filters(TEST_VIEW_ID_V1);
  let source_sorts_before: Vec<TestSort> = database_test.get_all_sorts(TEST_VIEW_ID_V1);
  let source_field_settings_before =
    database_test.get_field_settings::<TestFieldSetting>(TEST_VIEW_ID_V1, None);

  let duplicated_view_id = Uuid::new_v4();
  let duplicated_view_name = "Duplicated linked view".to_string();
  let duplicated_view = database_test
    .duplicate_linked_view_with_id(
      TEST_VIEW_ID_V1,
      duplicated_view_id,
      Some(duplicated_view_name.clone()),
    )
    .unwrap();
  let duplicated_view_id_str = duplicated_view_id.to_string();

  assert_eq!(duplicated_view.id, duplicated_view_id);
  assert_eq!(duplicated_view.name, duplicated_view_name);
  assert_eq!(duplicated_view.database_id, source_view_before.database_id);

  let duplicated_view_from_db = database_test.get_view(&duplicated_view_id_str).unwrap();
  let duplicated_order: Vec<String> = duplicated_view_from_db
    .field_orders
    .iter()
    .map(|field| field.id.clone())
    .collect();
  let duplicated_filters: Vec<TestFilter> = database_test.get_all_filters(&duplicated_view_id_str);
  let duplicated_sorts: Vec<TestSort> = database_test.get_all_sorts(&duplicated_view_id_str);
  let duplicated_field_settings =
    database_test.get_field_settings::<TestFieldSetting>(&duplicated_view_id_str, None);

  assert_eq!(duplicated_order, source_order_before);
  assert_eq!(duplicated_filters.len(), source_filters_before.len());
  assert_eq!(duplicated_sorts.len(), source_sorts_before.len());
  assert_eq!(
    duplicated_field_settings.get("f1").unwrap().width,
    source_field_settings_before.get("f1").unwrap().width
  );
  assert_eq!(
    duplicated_field_settings.get("f1").unwrap().visibility,
    source_field_settings_before.get("f1").unwrap().visibility
  );

  database_test.remove_all_filters(&duplicated_view_id_str);
  database_test.remove_all_sorts(&duplicated_view_id_str);
  database_test.update_field_settings(
    &duplicated_view_id_str,
    Some(vec!["f1".to_string()]),
    TestFieldSetting {
      width: 120,
      visibility: 0,
    },
  );
  database_test.update_database_view(&duplicated_view_id_str, |view| {
    view.move_field_order("f1", "f2");
  });

  let source_view_after = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  let source_order_after: Vec<String> = source_view_after
    .field_orders
    .iter()
    .map(|field| field.id.clone())
    .collect();
  let source_filters_after: Vec<TestFilter> = database_test.get_all_filters(TEST_VIEW_ID_V1);
  let source_sorts_after: Vec<TestSort> = database_test.get_all_sorts(TEST_VIEW_ID_V1);
  let source_field_settings_after =
    database_test.get_field_settings::<TestFieldSetting>(TEST_VIEW_ID_V1, None);

  assert_eq!(source_order_after, source_order_before);
  assert_eq!(source_filters_after.len(), source_filters_before.len());
  assert_eq!(source_sorts_after.len(), source_sorts_before.len());
  assert_eq!(
    source_field_settings_after.get("f1").unwrap().width,
    source_field_settings_before.get("f1").unwrap().width
  );
  assert_eq!(
    source_field_settings_after.get("f1").unwrap().visibility,
    source_field_settings_before.get("f1").unwrap().visibility
  );
}

#[tokio::test]
async fn duplicate_database_excludes_embedded_views_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;

  let embedded_view_id = Uuid::new_v4();
  database_test
    .create_linked_view(CreateViewParams {
      database_id,
      view_id: embedded_view_id,
      name: "embedded view".to_string(),
      embedded: true,
      ..Default::default()
    })
    .unwrap();

  let data = database_test
    .get_database_data(20, false, true)
    .await
    .unwrap();

  let non_embedded_view_id = data.views.iter().find(|v| !v.embedded).unwrap().id;

  let params_without_embedded = CreateDatabaseParams::from_database_data(
    data.clone(),
    non_embedded_view_id,
    Uuid::new_v4(),
    false,
  );
  assert_eq!(params_without_embedded.views.len(), 1);
  assert!(params_without_embedded.views.iter().all(|v| !v.embedded));

  let params_with_embedded =
    CreateDatabaseParams::from_database_data(data, non_embedded_view_id, Uuid::new_v4(), true);
  assert_eq!(params_with_embedded.views.len(), 2);
  assert!(params_with_embedded.views.iter().any(|v| v.embedded));
}

#[tokio::test]
async fn database_data_serde_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let database_data = database_test
    .get_database_data(20, false, false)
    .await
    .unwrap();

  let json = database_data.to_json().unwrap();
  let database_data2 = DatabaseData::from_json(&json).unwrap();
  assert_eq!(database_data.fields.len(), database_data2.fields.len());
  assert_eq!(database_data.rows.len(), database_data2.rows.len());
}

#[tokio::test]
async fn get_database_view_layout_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;

  let layout = database_test.get_database_view_layout(TEST_VIEW_ID_V1);
  assert_eq!(layout, DatabaseLayout::Grid);
}

#[tokio::test]
async fn update_database_view_layout_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  database_test.update_database_view(TEST_VIEW_ID_V1, |update| {
    update.set_layout_type(DatabaseLayout::Calendar);
  });

  let layout = database_test.get_database_view_layout(TEST_VIEW_ID_V1);
  assert_eq!(layout, DatabaseLayout::Calendar);
}

#[tokio::test]
async fn validate_database_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  assert!(database_test.database.validate().is_ok())
}
