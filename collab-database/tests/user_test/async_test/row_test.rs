use collab_database::database::timestamp;
use collab_database::rows::CreateRowParams;
use collab_database::rows::RowId;
use serde_json::{json, Value};

use collab_plugins::local_storage::CollabPersistenceConfig;
use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::helper::TestTextCell;
use crate::user_test::async_test::script::{create_database, database_test, DatabaseScript};

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
          cells: Cell::new()
            .insert_cell("f1", TestTextCell::from("hello world"))
            .build(),
        })
      }
      cloned_test.run_scripts(scripts).await;
    });
    handles.push(handle);
  }
  while handles.next().await.is_some() {}

  let mut expected = edit_row_expected();

  let timestamp = timestamp();
  expected["rows"][0]["cells"]["f1"]["data"] = Value::String("hello world".to_string());
  expected["rows"][0]["modified_at"] = Value::Number(timestamp.into());

  test
    .run_scripts(vec![
      DatabaseScript::IsExist {
        oid: database_id.clone(),
        expected: true,
      },
      DatabaseScript::AssertDatabaseInDisk {
        database_id: database_id.clone(),
        expected,
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
      let mut expected = create_row_test_expected();
      expected["views"][0]["database_id"] = Value::String(database_id.clone());
      expected["rows"][3]["created_at"] = Value::Number(timestamp().into());

      cloned_test
        .run_scripts(vec![DatabaseScript::AssertDatabaseInDisk {
          database_id: database_id.clone(),
          expected,
        }])
        .await;
    });
    handles.push(handle);
  }
  while handles.next().await.is_some() {}
}

fn edit_row_expected() -> Value {
  json!({
    "fields": [
      {
        "field_type": 0,
        "id": "f1",
        "is_primary": true,
        "name": "text field",
        "type_options": {},
        "visibility": true,
        "width": 120
      },
      {
        "field_type": 2,
        "id": "f2",
        "is_primary": true,
        "name": "single select field",
        "type_options": {},
        "visibility": true,
        "width": 120
      },
      {
        "field_type": 1,
        "id": "f3",
        "is_primary": true,
        "name": "checkbox field",
        "type_options": {},
        "visibility": true,
        "width": 120
      }
    ],
    "inline_view_id": "v1",
    "rows": [
      {
        "cells": {
          "f1": {
            "data": "1f1cell"
          },
          "f2": {
            "data": "1f2cell"
          },
          "f3": {
            "data": "1f3cell"
          }
        },
        "created_at": 1703772730,
        "height": 0,
        "id": "1",
        "visibility": true
      },
      {
        "cells": {
          "f1": {
            "data": "2f1cell"
          },
          "f2": {
            "data": "2f2cell"
          }
        },
        "created_at": 1703772730,
        "height": 0,
        "id": "2",
        "visibility": true
      },
      {
        "cells": {
          "f1": {
            "data": "3f1cell"
          },
          "f3": {
            "data": "3f3cell"
          }
        },
        "created_at": 1703772730,
        "height": 0,
        "id": "3",
        "visibility": true
      }
    ],
    "views": [
      {
        "database_id": "d2",
        "field_orders": [
          {
            "id": "f1"
          },
          {
            "id": "f2"
          },
          {
            "id": "f3"
          }
        ],
        "filters": [],
        "group_settings": [],
        "id": "v1",
        "layout": 0,
        "layout_settings": {},
        "name": "my first database view",
        "row_orders": [
          {
            "height": 0,
            "id": "1"
          },
          {
            "height": 0,
            "id": "2"
          },
          {
            "height": 0,
            "id": "3"
          }
        ],
        "sorts": []
      }
    ]
  })
}

fn create_row_test_expected() -> Value {
  json!(
  {
    "fields": [
      {
        "field_type": 0,
        "id": "f1",
        "is_primary": true,
        "name": "text field",
        "type_options": {},
        "visibility": true,
        "width": 120
      },
      {
        "field_type": 2,
        "id": "f2",
        "is_primary": true,
        "name": "single select field",
        "type_options": {},
        "visibility": true,
        "width": 120
      },
      {
        "field_type": 1,
        "id": "f3",
        "is_primary": true,
        "name": "checkbox field",
        "type_options": {},
        "visibility": true,
        "width": 120
      }
    ],
    "inline_view_id": "v1",
    "rows": [
      {
        "block_id": 1,
        "cells": {
          "f1": {
            "data": "1f1cell"
          },
          "f2": {
            "data": "1f2cell"
          },
          "f3": {
            "data": "1f3cell"
          }
        },
        "created_at": 1703772730,
        "height": 60,
        "id": "1",
        "visibility": true
      },
      {
        "block_id": 2,
        "cells": {
          "f1": {
            "data": "2f1cell"
          },
          "f2": {
            "data": "2f2cell"
          }
        },
        "created_at": 1703772730,
        "height": 60,
        "id": "2",
        "visibility": true
      },
      {
        "block_id": 3,
        "cells": {
          "f1": {
            "data": "3f1cell"
          },
          "f3": {
            "data": "3f3cell"
          }
        },
        "created_at": 1703772730,
        "height": 60,
        "id": "3",
        "visibility": true
      },
      {
        "block_id": 4,
        "cells": {},
        "created_at": 1703772730,
        "height": 60,
        "id": "4",
        "visibility": false
      }
    ],
    "views": [
      {
        "created_at": 0,
        "database_id": "d2",
        "field_orders": [
          {
            "id": "f1"
          },
          {
            "id": "f2"
          },
          {
            "id": "f3"
          }
        ],
        "filters": [],
        "group_settings": [],
        "id": "v1",
        "layout": 0,
        "layout_settings": {},
        "modified_at": 0,
        "name": "my first database view",
        "row_orders": [
          {
            "block_id": 1,
            "height": 0,
            "id": "1"
          },
          {
            "block_id": 2,
            "height": 0,
            "id": "2"
          },
          {
            "block_id": 3,
            "height": 0,
            "id": "3"
          },
          {
            "block_id": 4,
            "height": 0,
            "id": "4"
          }
        ],
        "sorts": []
      }
    ]
  }
  )
}
