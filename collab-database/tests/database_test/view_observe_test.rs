use crate::database_test::helper::{create_database, wait_for_specific_event};
use crate::helper::{setup_log, TestFieldSetting};
use collab_database::database::gen_row_id;

use collab_database::rows::CreateRowParams;
use collab_database::views::{CreateViewParams, DatabaseLayout, DatabaseViewChange};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn observer_delete_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id = gen_row_id();
  let cloned_row_id = row_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams::new(gen_row_id(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(
        cloned_row_id.clone(),
        database_id.clone(),
      ))
      .unwrap();
    cloned_database_test.remove_row(&cloned_row_id);
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { index } => {
      assert_eq!(index.len(), 1);
      index[0] == 1u32
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_delete_consecutive_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;

    cloned_database_test
      .create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
      .unwrap();

    cloned_database_test.remove_rows(&[row_id_2, row_id_3]);
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { index } => {
      assert_eq!(index.len(), 2);
      index[0] == 1u32 && index[1] == 2u32
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_delete_non_consecutive_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
      .unwrap();

    cloned_database_test.remove_rows(&[row_id_2, row_id_4]);
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { index } => {
      assert_eq!(index.len(), 2);
      index[0] == 1u32 && index[1] == 3u32
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_update_view_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();
  let cloned_database_test = database_test.clone();
  let view_id = database_test.get_inline_view_id();

  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .views
      .update_database_view(&view_id, |update| {
        update.set_name("hello");
      });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateView { view } => view.name == "hello",
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_create_delete_view_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();
  let create_view_id = uuid::Uuid::new_v4().to_string();
  let params = CreateViewParams {
    database_id: database_id.clone(),
    view_id: create_view_id.clone(),
    name: "my second grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };

  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test.create_linked_view(params).unwrap();
  });
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidCreateView { view } => view.name == "my second grid",
    _ => false,
  })
  .await
  .unwrap();

  let cloned_database_test = database_test.clone();
  let view_change_rx = database_test.subscribe_view_change();
  let view_id = create_view_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test.delete_view(&view_id);
  });
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteView { view_id } => view_id == &create_view_id,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_field_setting_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database(1, &database_id).await);
  let view_change_rx = database_test.subscribe_view_change();

  let cloned_database_test = database_test.clone();
  let view_id = database_test.get_inline_view_id();

  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let field_settings = TestFieldSetting {
      width: 10000,
      visibility: 1,
    };
    cloned_database_test.update_field_settings(&view_id, None, field_settings);
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateView { view: _ } => true,
    _ => false,
  })
  .await
  .unwrap();
}
