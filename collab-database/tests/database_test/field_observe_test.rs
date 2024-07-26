use crate::database_test::helper::{create_database_with_default_data, wait_for_specific_event};
use crate::helper::setup_log;
use collab_database::fields::FieldChange;

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn observe_field_update_and_delete_test() {
  setup_log();
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = Arc::new(create_database_with_default_data(1, &database_id).await);

  let field = database_test.get_fields(None).pop().unwrap();

  // Update
  let cloned_field = field.clone();
  let cloned_database_test = database_test.clone();
  tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    let mut lock = cloned_database_test.get_collab().lock().await;
    let mut txn = lock.transact_mut();
    cloned_database_test
      .fields
      .update_field(&mut txn, &cloned_field.id, |update| {
        update.set_name("hello world");
      });
  });

  let field_change_rx = database_test.subscribe_field_change();
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
    cloned_database_test.delete_field(&cloned_field.id);
  });

  let cloned_field = field.clone();
  let field_change_rx = database_test.subscribe_field_change();
  wait_for_specific_event(field_change_rx, |event| match event {
    FieldChange::DidDeleteField { field_id } => field_id == &cloned_field.id,
    _ => false,
  })
  .await
  .unwrap();
}
