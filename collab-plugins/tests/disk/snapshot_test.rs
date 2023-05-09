use crate::disk::script::CollabPersistenceTest;
use crate::disk::script::Script::*;
use collab_plugins::disk::rocksdb::Config;
use serde_json::json;

#[test]
fn disable_snapshot_test() {
  let mut test = CollabPersistenceTest::new(Config::new().enable_snapshot(false));
  let doc_id = "1".to_string();
  test.run_scripts(vec![OpenDocument { id: doc_id.clone() }]);

  for i in 1..=20 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    });
  }

  test.run_scripts(vec![
    AssertNumOfUpdates {
      id: doc_id.clone(),
      expected: 20,
    },
    AssertNumOfSnapshots {
      id: doc_id,
      expected: 0,
    },
  ]);
}

#[test]
fn gen_snapshot_after_load_from_disk_test() {
  let mut test = CollabPersistenceTest::new(
    Config::new()
      .enable_snapshot(true)
      .snapshot_per_update(5)
      .remove_updates_after_snapshot(true),
  );
  let doc_id = "1".to_string();
  test.run_scripts(vec![OpenDocument { id: doc_id.clone() }]);
  for i in 1..=9 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    });
  }
  test.run_scripts(vec![
    AssertNumOfUpdates {
      id: doc_id.clone(),
      expected: 4,
    },
    AssertNumOfSnapshots {
      id: doc_id.clone(),
      expected: 1,
    },
    CloseDocument { id: doc_id.clone() },
  ]);

  // reopen
  test.run_scripts(vec![OpenDocument { id: doc_id.clone() }]);
  test.run_scripts(vec![
    AssertNumOfUpdates {
      id: doc_id.clone(),
      expected: 4,
    },
    AssertNumOfSnapshots {
      id: doc_id.clone(),
      expected: 1,
    },
  ]);

  for i in 1..=2 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    });
  }

  test.run_scripts(vec![
    AssertNumOfUpdates {
      id: doc_id.clone(),
      expected: 1,
    },
    AssertNumOfSnapshots {
      id: doc_id,
      expected: 2,
    },
  ]);
}

#[test]
fn remove_updates_after_each_snapshot_test() {
  let snapshot_per_update = 5;
  let mut test = CollabPersistenceTest::new(
    Config::new()
      .enable_snapshot(true)
      .snapshot_per_update(snapshot_per_update)
      .remove_updates_after_snapshot(true),
  );
  let doc_id = "1".to_string();
  test.run_scripts(vec![OpenDocument { id: doc_id.clone() }]);

  for i in 1..=20 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    });

    if i != 1 && i % snapshot_per_update == 1 {
      test.run_scripts(vec![
        ValidateSnapshot {
          id: doc_id.clone(),
          snapshot_index: (i / 5) as usize - 1,
        },
        // When remove_updates_after_snapshot is true, the number of updates should be
        // removed. So the number of updates should be 1.
        AssertNumOfUpdates {
          id: doc_id.clone(),
          expected: 1,
        },
      ]);
    }
  }

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 0,
    expected: json!( {
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
      "5": 5.0,
    }),
  });

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 1,
    expected: json!( {
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
      "5": 5.0,
      "6": 6.0,
      "7": 7.0,
      "8": 8.0,
      "9": 9.0,
      "10": 10.0,
    }),
  });

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 2,
    expected: json!( {
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
      "5": 5.0,
      "6": 6.0,
      "7": 7.0,
      "8": 8.0,
      "9": 9.0,
      "10": 10.0,
      "11": 11.0,
      "12": 12.0,
      "13": 13.0,
      "14": 14.0,
      "15": 15.0,
    }),
  });

  test.run_scripts(vec![
    AssertNumOfSnapshots {
      id: doc_id.clone(),
      expected: 3,
    },
    AssertNumOfUpdates {
      id: doc_id.clone(),
      expected: 5,
    },
    AssertDocument {
      id: doc_id,
      expected: json!( {
        "1": 1.0,
        "2": 2.0,
        "3": 3.0,
        "4": 4.0,
        "5": 5.0,
        "6": 6.0,
        "7": 7.0,
        "8": 8.0,
        "9": 9.0,
        "10": 10.0,
        "11": 11.0,
        "12": 12.0,
        "13": 13.0,
        "14": 14.0,
        "15": 15.0,
        "16": 16.0,
        "17": 17.0,
        "18": 18.0,
        "19": 19.0,
        "20": 20.0,
      }),
    },
  ]);
}

#[test]
fn gen_snapshot_test() {
  let snapshot_per_update = 5;
  let mut test = CollabPersistenceTest::new(
    Config::new()
      .enable_snapshot(true)
      .snapshot_per_update(snapshot_per_update),
  );
  let doc_id = "1".to_string();
  test.run_scripts(vec![OpenDocument { id: doc_id.clone() }]);

  for i in 0..20 {
    test.run_script(InsertKeyValue {
      id: doc_id.clone(),
      key: i.to_string(),
      value: i.into(),
    });

    if i != 0 && i % snapshot_per_update == 0 {
      test.run_scripts(vec![
        ValidateSnapshot {
          id: doc_id.clone(),
          snapshot_index: (i / 5) as usize - 1,
        },
        AssertNumOfUpdates {
          id: doc_id.clone(),
          expected: i as usize + 1,
        },
      ]);
    }
  }

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 0,
    expected: json!( {
      "0": 0.0,
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
    }),
  });

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 1,
    expected: json!( {
      "0": 0.0,
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
      "5": 5.0,
      "6": 6.0,
      "7": 7.0,
      "8": 8.0,
      "9": 9.0,
    }),
  });

  test.run_script(AssertSnapshot {
    id: doc_id.clone(),
    index: 2,
    expected: json!( {
      "0": 0.0,
      "1": 1.0,
      "2": 2.0,
      "3": 3.0,
      "4": 4.0,
      "5": 5.0,
      "6": 6.0,
      "7": 7.0,
      "8": 8.0,
      "9": 9.0,
      "10": 10.0,
      "11": 11.0,
      "12": 12.0,
      "13": 13.0,
      "14": 14.0,
    }),
  });

  test.run_scripts(vec![
    AssertNumOfSnapshots {
      id: doc_id.clone(),
      expected: 3,
    },
    AssertNumOfUpdates {
      id: doc_id,
      expected: 20,
    },
  ]);
}
