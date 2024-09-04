use crate::database_test::helper::{create_database, wait_for_specific_event};
use crate::helper::setup_log;
use collab_database::database::gen_row_id;

use collab::lock::Mutex;
use collab_database::entity::CreateViewParams;
use collab_database::rows::CreateRowParams;
use collab_database::views::{
  DatabaseLayout, DatabaseViewChange, FilterMapBuilder, GroupSettingBuilder, SortMapBuilder,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn observer_delete_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id = gen_row_id();
  let cloned_row_id = row_id.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(gen_row_id(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(
      cloned_row_id.clone(),
      database_id.clone(),
    ))
    .await
    .unwrap();
    db.remove_row(&cloned_row_id).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { indexs } => {
      assert_eq!(indexs.len(), 1);
      indexs[0] == 1u32
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_delete_consecutive_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;

    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
      .await
      .unwrap();

    db.remove_rows(&[row_id_2, row_id_3]).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { indexs } => {
      assert_eq!(indexs.len(), 2);
      indexs[0] == 1u32 && indexs[1] == 2u32
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_delete_non_consecutive_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
      .await
      .unwrap();
    db.create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
      .await
      .unwrap();

    db.remove_rows(&[row_id_2, row_id_4]).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteRowAtIndex { indexs } => {
      assert_eq!(indexs.len(), 2);
      indexs[0] == 1u32 && indexs[1] == 3u32
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
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let view_id = database_test.get_inline_view_id();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&view_id, |update| {
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
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let create_view_id = uuid::Uuid::new_v4().to_string();
  let params = CreateViewParams {
    database_id: database_id.clone(),
    view_id: create_view_id.clone(),
    name: "my second grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .lock()
      .await
      .database
      .create_linked_view(params)
      .unwrap();
  });
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidCreateView { view } => view.name == "my second grid",
    _ => false,
  })
  .await
  .unwrap();

  let cloned_database_test = database_test.clone();
  let view_change_rx = database_test.lock().await.subscribe_view_change();
  let view_id = create_view_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .lock()
      .await
      .database
      .delete_view(&view_id);
  });
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidDeleteView { view_id } => view_id == &create_view_id,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_database_view_layout_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let update_view_id = database_test.get_inline_view_id();
  let cloned_update_view_id = update_view_id.clone();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      update.set_layout_type(DatabaseLayout::Calendar);
    });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::LayoutSettingChanged {
      view_id,
      layout_type,
    } => &update_view_id == view_id && layout_type == &DatabaseLayout::Calendar,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_database_view_filter_create_delete_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let update_view_id = database_test.get_inline_view_id();

  let database_test = Arc::new(Mutex::from(database_test));

  // create filter
  let cloned_database_test = database_test.clone();
  let cloned_update_view_id = update_view_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      let filter = FilterMapBuilder::from([("filter_id".into(), "123".into())]);
      update.set_filters(vec![filter]);
    });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidCreateFilters { view_id, filters } => {
      filters.len() == 1 && &update_view_id == view_id
    },
    _ => false,
  })
  .await
  .unwrap();

  // delete filter
  let cloned_update_view_id = update_view_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      update.set_filters(vec![]);
    });
  });

  let view_change_rx = database_test.lock().await.database.subscribe_view_change();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateFilter { view_id } => &update_view_id == view_id,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_database_view_sort_create_delete_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let update_view_id = database_test.get_inline_view_id();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();

  // create sort
  let cloned_update_view_id = update_view_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      let filter = SortMapBuilder::from([
        ("sort_id".into(), "123".into()),
        ("desc".into(), "true".into()),
      ]);
      update.set_sorts(vec![filter]);
    });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidCreateSorts { view_id, sorts } => {
      sorts.len() == 1 && &update_view_id == view_id
    },
    _ => false,
  })
  .await
  .unwrap();

  // delete sort
  let cloned_update_view_id = update_view_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      update.set_sorts(vec![]);
    });
  });

  let view_change_rx = database_test.lock().await.database.subscribe_view_change();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateSort { view_id } => &update_view_id == view_id,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_database_view_group_create_delete_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change();
  let update_view_id = database_test.get_inline_view_id();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();

  // create group setting
  let cloned_update_view_id = update_view_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      let group_setting = GroupSettingBuilder::from([
        ("group_id".into(), "123".into()),
        ("desc".into(), "true".into()),
      ]);
      update.set_groups(vec![group_setting]);
    });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidCreateGroupSettings { view_id, groups } => {
      groups.len() == 1 && &update_view_id == view_id
    },
    _ => false,
  })
  .await
  .unwrap();

  // delete group setting
  let cloned_update_view_id = update_view_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_database_view(&cloned_update_view_id, |update| {
      update.set_groups(vec![]);
    });
  });

  let view_change_rx = database_test.lock().await.database.subscribe_view_change();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateGroupSetting { view_id } => &update_view_id == view_id,
    _ => false,
  })
  .await
  .unwrap();
}
