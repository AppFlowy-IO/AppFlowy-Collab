use crate::helper::{create_user_database, poll_row_relation_rx, test_timeout};
use collab::preclude::MapRefExtension;
use collab_database::user::{RowRelation, RowRelationChange};

#[test]
fn insert_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  relations.with_transact_mut(|txn| {
    relations.insert_with_txn(txn, "version", "1.0");
  });

  let txn = relations.transact();
  assert_eq!(relations.get_str_with_txn(&txn, "version").unwrap(), "1.0");
}

#[test]
fn restore_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  relations.with_transact_mut(|txn| {
    relations.insert_with_txn(txn, "version", "1.0");
  });

  let database = user_db.open_user_database();
  let relations = database.relations();
  {
    let txn = relations.transact();
    assert_eq!(relations.get_str_with_txn(&txn, "version").unwrap(), "1.0");
  }

  relations.with_transact_mut(|txn| {
    relations.insert_with_txn(txn, "version", "2.0");
  });
}

#[tokio::test]
async fn insert_row_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  let mut rx = poll_row_relation_rx(relations.subscript_update());

  relations.insert_relation(RowRelation {
    linking_database_id: "d1".to_string(),
    linked_by_database_id: "d2".to_string(),
    row_connections: Default::default(),
  });

  // observe the update
  let value = test_timeout(rx.recv()).await.unwrap();
  match value {
    RowRelationChange::NewRelation(value) => {
      assert_eq!(value.linking_database_id, "d1");
      assert_eq!(value.linked_by_database_id, "d2");
    },
    RowRelationChange::DeleteRelation(_) => {},
  }
}

#[tokio::test]
async fn remove_row_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  let mut rx = poll_row_relation_rx(relations.subscript_update());

  let relation = RowRelation {
    linking_database_id: "d1".to_string(),
    linked_by_database_id: "d2".to_string(),
    row_connections: Default::default(),
  };
  relations.insert_relation(relation.clone());
  relations.remove_relation(&relation.id());

  // observe the update
  let value = test_timeout(rx.recv()).await.unwrap();
  match value {
    RowRelationChange::NewRelation(value) => {
      assert_eq!(value.linking_database_id, "d1");
      assert_eq!(value.linked_by_database_id, "d2");
    },
    RowRelationChange::DeleteRelation(_) => {},
  }
}
