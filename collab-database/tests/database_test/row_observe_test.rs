use std::sync::Arc;
use std::time::Duration;

use collab::lock::Mutex;
use tokio::time::sleep;

use collab::util::AnyMapExt;
use collab_database::database::gen_row_id;
use collab_database::rows::{Cell, CreateRowParams, RowChange, new_cell_builder};
use collab_database::views::DatabaseViewChange;

use crate::database_test::helper::{create_database, wait_for_specific_event};

#[tokio::test]
async fn observer_create_new_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id = gen_row_id();
  let cloned_row_id = row_id.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let row = CreateRowParams::new(cloned_row_id, database_id.clone());
    cloned_database_test
      .lock()
      .await
      .create_row(row)
      .await
      .unwrap();
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidUpdateRowOrders {
      insert_row_orders, ..
    } => insert_row_orders.len() == 1 && insert_row_orders[0].0.id == row_id,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_row_cell_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let row_change_rx = database_test.subscribe_row_change().unwrap();
  let row_id = gen_row_id();

  // Insert cell
  let cloned_row_id = row_id.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(
      cloned_row_id.clone(),
      database_id.clone(),
    ))
    .await
    .unwrap();

    db.update_row(cloned_row_id, |row| {
      row.update_cells(|cells| {
        cells.insert_cell(
          "f1",
          Cell::from([("level".into(), 1.into()), ("field_type".into(), 1.into())]),
        );
      });
    })
    .await;
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateCell {
      row_id: _,
      field_id,
      value,
    } => field_id == "f1" && value.get_as::<i64>("level") == Some(1),
    _ => false,
  })
  .await
  .unwrap();

  // Update cell
  let cloned_database_test = database_test.clone();
  let row_change_rx = database_test
    .lock()
    .await
    .database
    .subscribe_row_change()
    .unwrap();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;

    let mut db = cloned_database_test.lock().await;
    db.update_row(row_id, |row| {
      row.update_cells(|cells| {
        cells.insert_cell("f1", {
          let mut cell = new_cell_builder(1);
          cell.insert("level".into(), 2.into());
          cell
        });
      });
    })
    .await;
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateCell {
      row_id: _,
      field_id,
      value,
    } => field_id == "f1" && value.get_as::<i64>("level") == Some(2),
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_update_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id);
  let row_change_rx = database_test.subscribe_row_change().unwrap();

  let row_id = gen_row_id();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.create_row(CreateRowParams::new(row_id.clone(), database_id.clone()))
      .await
      .unwrap();

    db.update_row(row_id, |row| {
      row.set_height(1000);
    })
    .await;
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateHeight { row_id: _, value } => *value == 1000i32,
    _ => false,
  })
  .await
  .unwrap();
}
