use collab_plugins::disk::rocksdb::CollabPersistenceConfig;
use serde_json::{json, Value};

use crate::user_test::async_test::script::{create_database, database_test, DatabaseScript::*};

#[tokio::test]
async fn flush_doc_test() {
  let mut test = database_test(CollabPersistenceConfig::new().flush_doc(true));
  test
    .run_scripts(vec![
      CreateDatabase {
        params: create_database("d1"),
      },
      AssertNumOfUpdates {
        oid: "d1".to_string(),
        expected: 2,
      },
      CloseDatabase {
        database_id: "d1".to_string(),
      },
      AssertDatabase {
        database_id: "d1".to_string(),
        expected: expect(),
      },
    ])
    .await;

  test
    .run_scripts(vec![
      OpenDatabase {
        database_id: "d1".to_string(),
      },
      AssertNumOfUpdates {
        oid: "d1".to_string(),
        expected: 0,
      },
      AssertDatabase {
        database_id: "d1".to_string(),
        expected: expect(),
      },
    ])
    .await;
}

fn expect() -> Value {
  json!( {
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
    "inline_view": "v1",
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
        "created_at": 0,
        "height": 60,
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
        "created_at": 0,
        "height": 60,
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
        "created_at": 0,
        "height": 60,
        "id": "3",
        "visibility": true
      }
    ],
    "views": [
      {
        "database_id": "d1",
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
        "name": "my first database",
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
