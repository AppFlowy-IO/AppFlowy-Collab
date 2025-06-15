use crate::database_test::helper::{create_database, wait_for_specific_event};
use crate::helper::setup_log;
use collab_database::database::{DatabaseBody, gen_row_id};

use collab::lock::Mutex;
use collab_database::database_trait::NoPersistenceDatabaseCollabService;
use collab_database::entity::CreateViewParams;
use collab_database::rows::CreateRowParams;
use collab_database::views::{
  DatabaseLayout, DatabaseViewChange, FilterMapBuilder, GroupSettingBuilder, SortMapBuilder,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn observer_delete_consecutive_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_row_id_2 = row_id_2.clone();
  let cloned_row_id_3 = row_id_3.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  {
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
  }

  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(500)).await;
    let mut db = cloned_database_test.lock().await;
    db.remove_rows(&[cloned_row_id_2, cloned_row_id_3]).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      delete_row_indexes, ..
    } => {
      if delete_row_indexes.len() != 2 {
        false
      } else {
        assert_eq!(delete_row_indexes.len(), 2);
        delete_row_indexes[0] == 1u32 && delete_row_indexes[1] == 2u32
      }
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_row_id_2 = row_id_2.clone();
  let cloned_row_id_4 = row_id_4.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  {
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
  }

  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(500)).await;
    let mut db = cloned_database_test.lock().await;
    db.remove_rows(&[cloned_row_id_2, cloned_row_id_4]).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      delete_row_indexes, ..
    } => {
      if delete_row_indexes.len() != 2 {
        false
      } else {
        assert_eq!(delete_row_indexes.len(), 2);
        delete_row_indexes[0] == 1u32 && delete_row_indexes[1] == 3u32
      }
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_move_database_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
    .await
    .unwrap();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  let cloned_row_id_1 = row_id_1.clone();

  let views = database_test.lock().await.get_all_views();
  assert_eq!(views.len(), 1);
  let view_id = views[0].id.clone();
  let row_orders = database_test.lock().await.get_row_orders_for_view(&view_id);
  assert_eq!(row_orders.len(), 4);
  assert_eq!(row_orders[0].id, row_id_1);
  assert_eq!(row_orders[1].id, row_id_2);
  assert_eq!(row_orders[2].id, row_id_3);
  assert_eq!(row_orders[3].id, row_id_4);

  let cloned_row_id_3 = row_id_3.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(500)).await;
    let mut db = cloned_database_test.lock().await;
    // [row_id_1, row_id_2, row_id_3, row_id_4]
    db.move_row(&cloned_row_id_1, &cloned_row_id_3).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      insert_row_orders,
      delete_row_indexes,
      ..
    } => {
      if delete_row_indexes.len() == 1 {
        // [row_id_1, row_id_2, row_id_3, row_id_1, row_id_4]
        // after apply delete_row_indexs and insert_row_orders, then the array will be
        // [row_id_2, row_id_3, row_id_1, row_id_4]
        assert_eq!(delete_row_indexes[0], 0);
        assert_eq!(insert_row_orders[0].0.id, row_id_1);
        assert_eq!(insert_row_orders[0].1, 3);
        true
      } else {
        false
      }
    },
    _ => false,
  })
  .await
  .unwrap();

  let row_orders = database_test.lock().await.get_row_orders_for_view(&view_id);
  assert_eq!(row_orders.len(), 4);
  assert_eq!(row_orders[0].id, row_id_2);
  assert_eq!(row_orders[1].id, row_id_3);
  assert_eq!(row_orders[2].id, row_id_1);
  assert_eq!(row_orders[3].id, row_id_4);

  let cloned_row_id_1 = row_id_1.clone();
  let cloned_row_id_2 = row_id_2.clone();
  let cloned_database_test = database_test.clone();
  let view_change_rx = database_test.lock().await.subscribe_view_change().unwrap();
  tokio::spawn(async move {
    sleep(Duration::from_millis(500)).await;
    let mut db = cloned_database_test.lock().await;
    // [row_id_2, row_id_3, row_id_1, row_id_4]
    db.move_row(&cloned_row_id_1, &cloned_row_id_2).await;
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      insert_row_orders,
      delete_row_indexes,
      ..
    } => {
      if delete_row_indexes.len() == 1 {
        // [row_id_1, row_id_2, row_id_3, row_id_1, row_id_4]
        // after apply delete_row_indexs and insert_row_orders, then the array will be
        // [row_id_1, row_id_2, row_id_3, row_id_4]
        assert_eq!(delete_row_indexes[0], 3);
        assert_eq!(insert_row_orders[0].0.id, row_id_1);
        assert_eq!(insert_row_orders[0].1, 0);
        true
      } else {
        false
      }
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observe_move_database_view_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(row_id_1.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_2.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_3.clone(), database_id.clone()))
    .await
    .unwrap();
  database_test
    .create_row(CreateRowParams::new(row_id_4.clone(), database_id.clone()))
    .await
    .unwrap();

  let second_view_id = uuid::Uuid::new_v4().to_string();
  let first_view_id = database_test.get_all_views().first().unwrap().id.clone();
  database_test
    .create_linked_view(CreateViewParams {
      database_id: database_id.clone(),
      view_id: second_view_id.to_string(),
      name: "my second grid".to_string(),
      layout: DatabaseLayout::Grid,
      ..Default::default()
    })
    .unwrap();

  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  let cloned_row_id_1 = row_id_1.clone();

  let views = database_test.lock().await.get_all_views();
  assert_eq!(views.len(), 2);

  let cloned_row_id_3 = row_id_3.clone();
  let cloned_first_view_id = first_view_id.clone();
  println!(
    "first_view_id: {}, second_view_id:{}",
    first_view_id, second_view_id
  );
  tokio::spawn(async move {
    sleep(Duration::from_millis(500)).await;
    let mut db = cloned_database_test.lock().await;
    // [row_id_1, row_id_2, row_id_3, row_id_4]
    db.update_database_view(&cloned_first_view_id, |view| {
      view.move_row_order(&cloned_row_id_1, &cloned_row_id_3);
    });
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      insert_row_orders,
      delete_row_indexes,
      ..
    } => {
      if delete_row_indexes.len() == 1 {
        // [row_id_1, row_id_2, row_id_3, row_id_1, row_id_4]
        // after apply delete_row_indexs and insert_row_orders, then the array will be
        // [row_id_2, row_id_3, row_id_1, row_id_4]
        assert_eq!(delete_row_indexes[0], 0);
        assert_eq!(insert_row_orders[0].0.id, row_id_1);
        assert_eq!(insert_row_orders[0].1, 3);
        true
      } else {
        false
      }
    },
    _ => false,
  })
  .await
  .unwrap();

  // After move row, the row orders should be [row_id_2, row_id_3, row_id_1, row_id_4]
  // Get the row orders for given database view
  let row_orders = database_test
    .lock()
    .await
    .get_row_orders_for_view(&first_view_id);
  assert_eq!(row_orders.len(), 4);
  assert_eq!(row_orders[0].id, row_id_2);
  assert_eq!(row_orders[1].id, row_id_3);
  assert_eq!(row_orders[2].id, row_id_1);
  assert_eq!(row_orders[3].id, row_id_4);
  {
    let collab = &database_test.lock().await.collab;
    let db_body = DatabaseBody::from_collab(
      collab,
      Arc::new(NoPersistenceDatabaseCollabService::new(collab.client_id())),
      None,
    )
    .unwrap();

    let txn = collab.transact();
    let body_row_orders = db_body.views.get_row_orders(&txn, &first_view_id);
    assert_eq!(body_row_orders.len(), 4);
    assert_eq!(body_row_orders[0].id, row_id_2);
    assert_eq!(body_row_orders[1].id, row_id_3);
    assert_eq!(body_row_orders[2].id, row_id_1);
    assert_eq!(body_row_orders[3].id, row_id_4);
  }

  // Get row orders for the second view
  // Moving row in the first database view should not affect the row orders in the second view
  let second_row_orders = database_test
    .lock()
    .await
    .get_row_orders_for_view(&second_view_id);
  assert_eq!(second_row_orders.len(), 4);
  assert_eq!(
    second_row_orders.len(),
    4,
    "Expected 4 rows, got {:?}",
    second_row_orders
  );

  let expected_ids = [&row_id_1, &row_id_2, &row_id_3, &row_id_4];
  for (i, expected_id) in expected_ids.into_iter().enumerate() {
    assert_eq!(
      &second_row_orders[i].id, expected_id,
      "Mismatch at row {}: current row orders: {:?}",
      i, second_row_orders
    );
  }

  {
    let collab = &database_test.lock().await.collab;
    let db_body = DatabaseBody::from_collab(
      collab,
      Arc::new(NoPersistenceDatabaseCollabService::new(collab.client_id())),
      None,
    )
    .unwrap();

    let txn = collab.transact();
    let body_row_orders = db_body.views.get_row_orders(&txn, &second_view_id);
    assert_eq!(body_row_orders.len(), 4);
    assert_eq!(body_row_orders[0].id, row_id_1);
    assert_eq!(body_row_orders[1].id, row_id_2);
    assert_eq!(body_row_orders[2].id, row_id_3);
    assert_eq!(body_row_orders[3].id, row_id_4);
  }
}

#[tokio::test]
async fn observer_create_delete_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let created_row = vec![
    row_id_1.clone(),
    row_id_2.clone(),
    row_id_3.clone(),
    row_id_4.clone(),
  ];
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
  });

  let view_change_rx = database_test.lock().await.subscribe_view_change().unwrap();
  let mut received_rows = vec![];
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      database_view_id,
      is_local_change,
      insert_row_orders,
      delete_row_indexes,
    } => {
      if database_view_id == "v1" {
        assert!(is_local_change);
        assert_eq!(delete_row_indexes.len(), 0);
        for (row_order, index) in insert_row_orders {
          let pos = created_row.iter().position(|x| x == &row_order.id).unwrap() as u32;
          assert_eq!(&pos, index);
          received_rows.push(row_order.id.clone());
        }
        created_row == received_rows
      } else {
        false
      }
    },
    _ => false,
  })
  .await
  .unwrap();

  let cloned_database_test = database_test.clone();
  let cloned_created_row = created_row.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.move_row(&created_row[0], &created_row[2]).await;
  });

  let view_change_rx = database_test.lock().await.subscribe_view_change().unwrap();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      database_view_id,
      is_local_change,
      insert_row_orders,
      delete_row_indexes,
    } => {
      if database_view_id == "v1" {
        assert!(is_local_change);
        assert_eq!(delete_row_indexes.len(), 1);
        assert_eq!(delete_row_indexes[0], 0);

        assert_eq!(insert_row_orders.len(), 1);
        assert_eq!(insert_row_orders[0].0.id, cloned_created_row[0]);
        assert_eq!(insert_row_orders[0].1, 3);
        true
      } else {
        false
      }
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_create_row_when_reopen_a_database_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);

  let database_test = Arc::new(Mutex::from(database_test));
  let row_id = gen_row_id();
  let cloned_database_test = database_test.clone();
  let cloned_row_id = row_id.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(cloned_row_id, database_id.clone()))
      .await
      .unwrap();
  });

  let view_change_rx = database_test.lock().await.subscribe_view_change().unwrap();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      database_view_id,
      is_local_change: _,
      insert_row_orders,
      delete_row_indexes,
    } => {
      if database_view_id == "v1" {
        assert_eq!(insert_row_orders.len(), 1);
        assert_eq!(insert_row_orders[0].0.id, row_id);
        assert!(delete_row_indexes.is_empty());
        true
      } else {
        false
      }
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let view_id = database_test.get_first_database_view_id().unwrap();

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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
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
  let view_change_rx = database_test.lock().await.subscribe_view_change().unwrap();
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let update_view_id = database_test.get_first_database_view_id().unwrap();
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let update_view_id = database_test.get_first_database_view_id().unwrap();

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

  let view_change_rx = database_test
    .lock()
    .await
    .database
    .subscribe_view_change()
    .unwrap();
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let update_view_id = database_test.get_first_database_view_id().unwrap();

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

  let view_change_rx = database_test
    .lock()
    .await
    .database
    .subscribe_view_change()
    .unwrap();
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
  let view_change_rx = database_test.subscribe_view_change().unwrap();
  let update_view_id = database_test.get_first_database_view_id().unwrap();

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

  let view_change_rx = database_test
    .lock()
    .await
    .database
    .subscribe_view_change()
    .unwrap();
  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateGroupSetting { view_id } => &update_view_id == view_id,
    _ => false,
  })
  .await
  .unwrap();
}
