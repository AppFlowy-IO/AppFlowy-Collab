use crate::script::CollabPersistenceTest;
use crate::script::Script::*;
use serde_json::json;

#[test]
fn gen_snapshot_test() {
  let mut test = CollabPersistenceTest::new();
  let doc_id = "1".to_string();
  test.run_scripts(vec![OpenDocumentWithSnapshotPlugin { id: doc_id.clone() }]);

  for i in 0..20 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    })
  }

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 0,
    expected: json!( {
      "0": 0.0,
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0
    }),
  });
  //
  // test.run_script(AssertSnapshot {
  //   id: doc_id,
  //   index: 2,
  //   expected: json!({
  //     "0": 0.0,
  //     "1": 1.0,
  //     "10": 10.0,
  //     "11": 11.0,
  //     "12": 12.0,
  //     "13": 13.0,
  //     "14": 14.0,
  //     "15": 15.0,
  //     "16": 16.0,
  //     "17": 17.0,
  //     "18": 18.0,
  //     "2": 2.0,
  //     "3": 3.0,
  //     "4": 4.0,
  //     "5": 5.0,
  //     "6": 6.0,
  //     "7": 7.0,
  //     "8": 8.0,
  //     "9": 9.0
  //   }),
  // })
}
