use collab_database::database::timestamp;
use collab_database::rows::CreateRowParams;
use collab_database::rows::{CellsBuilder, RowId};
use serde_json::{json, Value};

use collab_plugins::local_storage::CollabPersistenceConfig;
use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::helper::TestTextCell;
use crate::user_test::async_test::script::{
  create_database, database_test, expected_fields, expected_rows, expected_view, DatabaseScript,
};

#[tokio::test]
async fn edit_row_test() {
  let mut test = database_test(CollabPersistenceConfig::default()).await;
  let mut handles = FuturesUnordered::new();
  let database_id = "d2".to_string();
  let row_id: RowId = 1.into();
  test
    .run_scripts(vec![
      DatabaseScript::IsExist {
        oid: database_id.clone(),
        expected: false,
      },
      DatabaseScript::CreateDatabase {
        params: create_database(&database_id),
      },
      // DatabaseScript::AssertNumOfUpdates {
      //   oid: "block_1".to_string(),
      //   expected: 2,
      // },
    ])
    .await;

  // spawn 10 task to edit the same row and each task edit the row 10 times.
  for _ in 0..10 {
    let mut cloned_test = test.clone();
    let cloned_database_id = database_id.clone();
    let cloned_row_id = row_id.clone();
    let handle = tokio::spawn(async move {
      let mut scripts = vec![];
      for _ in 0..10 {
        scripts.push(DatabaseScript::EditRow {
          database_id: cloned_database_id.clone(),
          row_id: cloned_row_id.clone(),
          cells: CellsBuilder::new()
            .insert_cell("f1", TestTextCell::from("hello world"))
            .build(),
        })
      }
      cloned_test.run_scripts(scripts).await;
    });
    handles.push(handle);
  }
  while handles.next().await.is_some() {}
  let timestamp = timestamp();

  let mut expected_rows = expected_rows();
  expected_rows[0]["cells"]["f1"]["data"] = Value::String("hello world".to_string());
  expected_rows[0]["last_modified"] = Value::Number(timestamp.into());
  let mut expected_view = expected_view();
  expected_view["database_id"] = Value::String(database_id.clone());

  test
    .run_scripts(vec![
      DatabaseScript::IsExist {
        oid: database_id.clone(),
        expected: true,
      },
      DatabaseScript::AssertDatabaseInDisk {
        database_id: database_id.clone(),
        expected_fields: expected_fields(),
        expected_rows,
        expected_view,
      },
      DatabaseScript::AssertNumOfUpdates {
        oid: database_id,
        expected: 2,
      },
      // DatabaseScript::AssertNumOfUpdates {
      //   oid: "block_1".to_string(),
      //   expected: 102,
      // },
    ])
    .await;
}

#[tokio::test]
async fn create_row_test() {
  let test = database_test(CollabPersistenceConfig::default()).await;
  let mut handles = FuturesUnordered::new();
  // Create 20 database and save them to disk in unordered.
  for i in 0..20 {
    let mut cloned_test = test.clone();
    let handle = tokio::spawn(async move {
      let database_id = format!("d{}", i);
      let mut scripts = vec![];
      scripts.push(DatabaseScript::CreateDatabase {
        params: create_database(&database_id),
      });

      for i in 4..5 {
        scripts.push(DatabaseScript::CreateRow {
          database_id: database_id.clone(),
          params: CreateRowParams {
            id: i.into(),
            cells: Default::default(),
            height: 0,
            visibility: false,
            ..Default::default()
          },
        });
      }
      cloned_test.run_scripts(scripts).await;
      let timestamp = timestamp();

      let mut expected_rows = expected_rows();
      expected_rows.as_array_mut().unwrap().push(json!({
        "cells": {},
        "created_at": timestamp,
        "last_modified": timestamp,
        "height": 0,
        "id": "4",
        "visibility": false
      }));
      let mut expected_view = expected_view();
      expected_view["database_id"] = Value::String(database_id.clone());
      expected_view["modified_at"] = Value::Number(timestamp.into());
      expected_view["row_orders"]
        .as_array_mut()
        .unwrap()
        .push(json!({
          "height": 0,
          "id": "4"
        }));

      cloned_test
        .run_scripts(vec![DatabaseScript::AssertDatabaseInDisk {
          database_id: database_id.clone(),
          expected_fields: expected_fields(),
          expected_rows,
          expected_view,
        }])
        .await;
    });
    handles.push(handle);
  }
  while handles.next().await.is_some() {}
}
