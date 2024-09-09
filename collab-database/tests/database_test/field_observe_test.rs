use crate::database_test::helper::{create_database_with_default_data, wait_for_specific_event};
use crate::helper::setup_log;
use collab_database::fields::FieldChange;

use collab::lock::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn observe_field_update_and_delete_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database_with_default_data(1, &database_id).await;

  let field = database_test.get_fields(None).pop().unwrap();

  // Update
  let cloned_field = field.clone();
  let database_test = Arc::new(Mutex::from(database_test));
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.update_field(&cloned_field.id, |update| {
      update.set_name("hello world");
    });
  });

  let field_change_rx = database_test.lock().await.subscribe_field_change().unwrap();
  wait_for_specific_event(field_change_rx, |event| match event {
    FieldChange::DidUpdateField { field } => field.name == "hello world",
    _ => false,
  })
  .await
  .unwrap();

  // delete
  let cloned_field = field.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut db = cloned_database_test.lock().await;
    db.delete_field(&cloned_field.id);
  });

  let cloned_field = field.clone();
  let field_change_rx = database_test.lock().await.subscribe_field_change().unwrap();
  wait_for_specific_event(field_change_rx, |event| match event {
    FieldChange::DidDeleteField { field_id } => field_id == &cloned_field.id,
    _ => false,
  })
  .await
  .unwrap();
}
