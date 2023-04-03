use crate::helper::create_user_database;
use collab::preclude::{Map, MapRefExtension, Transact};

#[test]
fn insert_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  relations.with_transact_mut(|txn| {
    relations.insert(txn, "version", "1.0");
  });

  let txn = relations.transact();
  assert_eq!(relations.get_str_with_txn(&txn, "version").unwrap(), "1.0");
}

#[test]
fn restore_relation_data_test() {
  let user_db = create_user_database(1);
  let relations = user_db.relations();
  relations.with_transact_mut(|txn| {
    relations.insert(txn, "version", "1.0");
  });

  // let database = user_db.open_user_database();
  // let relations = database.relations();
  // let txn = relations.transact();
  // assert_eq!(relations.get_str_with_txn(&txn, "version").unwrap(), "1.0");
}
