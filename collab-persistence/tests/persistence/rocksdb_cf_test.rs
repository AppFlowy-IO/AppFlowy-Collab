use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::kv::KVStore;

use crate::util::rocks_db;

#[tokio::test]
async fn open_same_cf_test() {
  let uid = 1;
  let (path, db_a) = rocks_db();
  db_a
    .with_write_txn(|txn| {
      txn.insert("1", "a")?;
      Ok(())
    })
    .unwrap();
  drop(db_a);

  let db_b = RocksCollabDB::open_with_cfs(vec![uid.to_string()], path).unwrap();
  let txn = db_b.read_txn();
  let value = txn.get("1").unwrap().unwrap();
  assert_eq!(value, "a".as_bytes());
}
