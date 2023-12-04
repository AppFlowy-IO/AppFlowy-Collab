use std::sync::Arc;
use std::time::Duration;

use collab::core::any_map::AnyMapExtension;
use collab_database::database::gen_row_id;
use collab_database::rows::{new_cell_builder, CreateRowParams, RowChange};
use collab_database::views::DatabaseViewChange;
use tokio::time::{sleep, timeout};

use crate::database_test::helper::create_database;

#[tokio::test]
async fn observer_create_new_row_test() {
  let database_test = Arc::new(create_database(1, "1").await);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id = gen_row_id();
  let cloned_row_id = row_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: cloned_row_id,
        ..Default::default()
      })
      .unwrap();
  });

  wait_for_specific_event(view_change_rx, |event| match event {
    DatabaseViewChange::DidInsertRowOrders { row_orders } => {
      row_orders.len() == 1 && row_orders[0].id == row_id
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_row_cell_test() {
  let database_test = Arc::new(create_database(1, "1").await);
  let row_change_rx = database_test.subscribe_row_change().unwrap();
  let row_id = gen_row_id();

  // Insert cell
  let cloned_row_id = row_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: cloned_row_id.clone(),
        ..Default::default()
      })
      .unwrap();

    cloned_database_test.update_row(&cloned_row_id, |row| {
      row.update_cells(|cells| {
        cells.insert_cell(
          "f1",
          new_cell_builder(1).insert_i64_value("level", 1).build(),
        );
      });
    });
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateCell { key, value } => {
      key == "f1" && value.get_i64_value("level") == Some(1)
    },
    _ => false,
  })
  .await
  .unwrap();

  // Update cell
  let cloned_database_test = database_test.clone();
  let row_change_rx = database_test.subscribe_row_change().unwrap();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;

    cloned_database_test.update_row(&row_id, |row| {
      row.update_cells(|cells| {
        cells.insert_cell(
          "f1",
          new_cell_builder(1).insert_i64_value("level", 2).build(),
        );
      });
    });
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateCell { key, value } => {
      key == "f1" && value.get_i64_value("level") == Some(2)
    },
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_update_row_test() {
  let database_test = Arc::new(create_database(1, "1").await);
  let row_change_rx = database_test.subscribe_row_change().unwrap();

  let row_id = gen_row_id();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id.clone(),
        ..Default::default()
      })
      .unwrap();

    cloned_database_test.update_row(&row_id, |row| {
      row.set_height(1000);
    });
  });

  wait_for_specific_event(row_change_rx, |event| match event {
    RowChange::DidUpdateHeight { value } => *value == 1000i32,
    _ => false,
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn observer_delete_row_test() {
  let database_test = Arc::new(create_database(1, "1").await);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id = gen_row_id();
  let cloned_row_id = row_id.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: gen_row_id(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: cloned_row_id.clone(),
        ..Default::default()
      })
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
  let database_test = Arc::new(create_database(1, "1").await);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_1.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_2.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_3.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_4.clone(),
        ..Default::default()
      })
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
  let database_test = Arc::new(create_database(1, "1").await);
  let view_change_rx = database_test.subscribe_view_change().unwrap();

  let row_id_1 = gen_row_id();
  let row_id_2 = gen_row_id();
  let row_id_3 = gen_row_id();
  let row_id_4 = gen_row_id();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_1.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_2.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_3.clone(),
        ..Default::default()
      })
      .unwrap();
    cloned_database_test
      .create_row(CreateRowParams {
        id: row_id_4.clone(),
        ..Default::default()
      })
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

async fn wait_for_specific_event<F, T>(
  mut change_rx: tokio::sync::broadcast::Receiver<T>,
  condition: F,
) -> Result<(), String>
where
  F: Fn(&T) -> bool,
  T: Clone,
{
  loop {
    let result = timeout(Duration::from_secs(5), change_rx.recv()).await;

    match result {
      Ok(Ok(event)) if condition(&event) => {
        // If the event matches the condition
        return Ok(());
      },
      Ok(Ok(_)) => {
        // If it's any other event, continue the loop
        continue;
      },
      Ok(Err(e)) => {
        // Channel error
        return Err(format!("Channel error: {}", e));
      },
      Err(e) => {
        // Timeout occurred
        return Err(format!("Timeout occurred: {}", e));
      },
    }
  }
}
