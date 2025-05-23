use crate::database_test::helper::{
  DatabaseTest, create_database_with_db, restore_database_from_db,
};
use crate::helper::unzip_history_database_db;
use assert_json_diff::{assert_json_eq, assert_json_include};

use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::Collab;
use collab_database::rows::CreateRowParams;
use collab_plugins::CollabKVDB;
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn restore_row_from_disk_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let database_id = uuid::Uuid::new_v4().to_string();
  let (db, mut database_test) = create_database_with_db(1, &workspace_id, &database_id).await;
  let row_1_id = Uuid::new_v4();
  let row_2_id = Uuid::new_v4();
  let row_1 = CreateRowParams::new(row_1_id, database_id.clone());
  let row_2 = CreateRowParams::new(row_2_id, database_id.clone());
  database_test.create_row(row_1.clone()).await.unwrap();
  database_test.create_row(row_2.clone()).await.unwrap();
  drop(database_test);

  let database_test = restore_database_from_db(1, &workspace_id, &database_id, db).await;
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 2);

  assert!(rows.iter().any(|row| row.id == row_1.id));
  assert!(rows.iter().any(|row| row.id == row_2.id));
}

#[tokio::test]
async fn restore_from_disk_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let database_id = Uuid::new_v4().to_string();
  let (db, database_test) = create_database_with_db(1, &workspace_id, &database_id).await;
  assert_database_eq(&database_id, database_test).await;

  // Restore from disk
  let database_test = restore_database_from_db(1, &workspace_id, &database_id, db).await;
  assert_database_eq(&database_id, database_test).await;
}

#[tokio::test]
async fn restore_from_disk_with_different_database_id_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let database_id = Uuid::new_v4().to_string();
  let (db, _) = create_database_with_db(1, &workspace_id, &database_id).await;
  let database_test = restore_database_from_db(1, &workspace_id, &database_id, db).await;

  assert_database_eq(&database_id, database_test).await;
}

#[tokio::test]
async fn restore_from_disk_with_different_uid_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let database_id = Uuid::new_v4().to_string();
  let (db, _) = create_database_with_db(1, &workspace_id, &database_id).await;
  let database_test = restore_database_from_db(1, &workspace_id, &database_id, db).await;

  assert_database_eq(&database_id, database_test).await;
}

async fn assert_database_eq(database_id: &str, database_test: DatabaseTest) {
  let expected = json!( {
    "fields": [],
    "rows": [],
    "views": [
      {
        "database_id": database_id,
        "field_orders": [],
        "filters": [],
        "group_settings": [],
        "layout": 0,
        "layout_settings": {},
        "row_orders": [],
        "sorts": []
      }
    ]
  });

  assert_json_include!(
    expected: expected,
    actual: database_test.to_json_value().await
  );
}

const HISTORY_DATABASE_020: &str = "020_database";

#[tokio::test]
async fn open_020_history_database_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let (_cleaner, db_path) = unzip_history_database_db(HISTORY_DATABASE_020).unwrap();
  let db = std::sync::Arc::new(CollabKVDB::open(db_path).unwrap());
  let database_test = restore_database_from_db(
    221439819971039232,
    &workspace_id,
    "c0e69740-49f0-4790-a488-702e2750ba8d",
    db,
  )
  .await;
  let actual_1 = database_test.to_json_value().await;
  assert_json_include!(expected: expected_json(), actual: actual_1);

  let bytes = std::fs::read("./tests/history_database/database_020_encode_collab").unwrap();
  let encode_collab = EncodedCollab::decode_from_bytes(&bytes).unwrap();
  let options = CollabOptions::new(
    "c0e69740-49f0-4790-a488-702e2750ba8d".to_string(),
    default_client_id(),
  )
  .with_data_source(encode_collab.into());
  let restored_database_collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let actual_2 = restored_database_collab.to_json_value();
  assert_json_eq!(expected_database_json(), actual_2);
}

fn expected_json() -> Value {
  json!({
    "fields": [
      {
        "field_type": 0,
        "id": "E_50ji",
        "is_primary": true,
        "name": "Name",
        "type_options": {
          "0": {
            "data": ""
          }
        }
      },
      {
        "field_type": 3,
        "id": "8tbGTb",
        "is_primary": false,
        "name": "Type",
        "type_options": {
          "3": {
            "content": "{\"options\":[{\"id\":\"jydv\",\"name\":\"3\",\"color\":\"LightPink\"},{\"id\":\"F2ew\",\"name\":\"2\",\"color\":\"Pink\"},{\"id\":\"hUJE\",\"name\":\"1\",\"color\":\"Purple\"}],\"disable_color\":false}"
          }
        }
      },
      {
        "field_type": 5,
        "id": "e-5TiR",
        "is_primary": false,
        "name": "Done",
        "type_options": {
          "5": {
            "is_selected": false
          }
        }
      },
      {
        "field_type": 1,
        "id": "QfCqmc",
        "is_primary": false,
        "name": "Text",
        "type_options": {
          "0": {
            "data": "",
            "format": 0,
            "name": "Number",
            "scale": 0,
            "symbol": "RUB"
          },
          "1": {
            "format": 1,
            "name": "Number",
            "scale": 0,
            "symbol": "RUB"
          }
        }
      },
      {
        "field_type": 6,
        "id": "vdCF8I",
        "is_primary": false,
        "name": "Text",
        "type_options": {
          "0": {
            "content": "",
            "data": "",
            "url": ""
          },
          "6": {
            "content": "",
            "url": ""
          }
        }
      },
      {
        "field_type": 8,
        "id": "9U02fU",
        "is_primary": false,
        "name": "Text",
        "type_options": {
          "0": {
            "data": "",
            "date_format": 3,
            "field_type": 8,
            "time_format": 0,
            "timezone_id": ""
          },
          "8": {
            "date_format": 3,
            "field_type": 8,
            "time_format": 0,
            "timezone_id": ""
          }
        }
      }
    ],
    "rows": [
      {
        "cells": {
          "8tbGTb": {
            "created_at": 1690639663,
            "data": "hUJE",
            "field_type": 3,
            "last_modified": 1690639663
          },
          "E_50ji": {
            "created_at": 1690639669,
            "data": "1",
            "field_type": 0,
            "last_modified": 1690639669
          },
          "QfCqmc": {
            "created_at": 1690639678,
            "data": "$1",
            "field_type": 1,
            "last_modified": 1690639678
          },
          "e-5TiR": {
            "created_at": 1690639660,
            "data": "Yes",
            "field_type": 5,
            "last_modified": 1690639660
          }
        },
        "height": 60,
        "id": "bbd404d8-1319-4e4d-84fe-1052c57fe3e7",
        "created_at": 1690639659,
        "modified_at": 1690639678,
        "visibility": true
      },
      {
        "cells": {
          "8tbGTb": {
            "created_at": 1690639665,
            "data": "F2ew",
            "field_type": 3,
            "last_modified": 1690639665
          },
          "E_50ji": {
            "created_at": 1690639669,
            "data": "2",
            "field_type": 0,
            "last_modified": 1690639669
          },
          "QfCqmc": {
            "created_at": 1690639679,
            "data": "$2",
            "field_type": 1,
            "last_modified": 1690639679
          },
          "e-5TiR": {
            "created_at": 1690639661,
            "data": "Yes",
            "field_type": 5,
            "last_modified": 1690639661
          }
        },
        "height": 60,
        "id": "bcfe322e-6272-4ed3-a57e-09645ec1073a",
        "created_at": 1690639659,
        "modified_at": 1690639679,
        "visibility": true
      },
      {
        "cells": {
          "8tbGTb": {
            "created_at": 1690639667,
            "data": "jydv",
            "field_type": 3,
            "last_modified": 1690639667
          },
          "E_50ji": {
            "created_at": 1690639670,
            "data": "3",
            "field_type": 0,
            "last_modified": 1690639670
          },
          "QfCqmc": {
            "created_at": 1690639679,
            "data": "$3",
            "field_type": 1,
            "last_modified": 1690639679
          },
          "e-5TiR": {
            "created_at": 1690639661,
            "data": "Yes",
            "field_type": 5,
            "last_modified": 1690639661
          }
        },
        "height": 60,
        "id": "5d4418d2-621a-4ac5-ad05-e2c6fcc1bc79",
        "created_at": 1690639659,
        "modified_at": 1690639679,
        "visibility": true
      }
    ],
    "views": [
      {
        "created_at": 1690639659,
        "database_id": "c0e69740-49f0-4790-a488-702e2750ba8d",
        "field_orders": [
          {
            "id": "E_50ji"
          },
          {
            "id": "8tbGTb"
          },
          {
            "id": "e-5TiR"
          },
          {
            "id": "QfCqmc"
          },
          {
            "id": "vdCF8I"
          },
          {
            "id": "9U02fU"
          }
        ],
        "filters": [
          {
            "condition": 2,
            "content": "",
            "field_id": "E_50ji",
            "id": "OWu470",
            "ty": 0
          }
        ],
        "group_settings": [],
        "id": "b44b2906-4508-4532-ad9e-2cf33ceae304",
        "layout": 0,
        "layout_settings": {},
        "modified_at": 1690639708,
        "name": "Untitled",
        "row_orders": [
          {
            "height": 60,
            "id": "bbd404d8-1319-4e4d-84fe-1052c57fe3e7"
          },
          {
            "height": 60,
            "id": "bcfe322e-6272-4ed3-a57e-09645ec1073a"
          },
          {
            "height": 60,
            "id": "5d4418d2-621a-4ac5-ad05-e2c6fcc1bc79"
          }
        ],
        "sorts": [
            {
              "condition": 0,
              "field_id": "E_50ji",
              "id": "s:4SJjUs",
              "ty": 0
            }
          ],
          "field_settings": {}
      }
    ]
  })
}

fn expected_database_json() -> Value {
  json!({
    "database": {
      "fields": {
        "8tbGTb": {
          "created_at": 1690639659,
          "id": "8tbGTb",
          "is_primary": false,
          "last_modified": 1690639667,
          "name": "Type",
          "ty": 3,
          "type_option": {
            "3": {
              "content": "{\"options\":[{\"id\":\"jydv\",\"name\":\"3\",\"color\":\"LightPink\"},{\"id\":\"F2ew\",\"name\":\"2\",\"color\":\"Pink\"},{\"id\":\"hUJE\",\"name\":\"1\",\"color\":\"Purple\"}],\"disable_color\":false}"
            }
          },
          "visibility": true,
          "width": 150
        },
        "9U02fU": {
          "created_at": 1690639699,
          "id": "9U02fU",
          "is_primary": false,
          "last_modified": 1690639702,
          "name": "Text",
          "ty": 8,
          "type_option": {
            "0": {
              "data": "",
              "date_format": 3,
              "field_type": 8,
              "time_format": 0,
              "timezone_id": ""
            },
            "8": {
              "date_format": 3,
              "field_type": 8,
              "time_format": 0,
              "timezone_id": ""
            }
          },
          "visibility": true,
          "width": 120
        },
        "E_50ji": {
          "created_at": 1690639659,
          "id": "E_50ji",
          "is_primary": true,
          "last_modified": 1690639659,
          "name": "Name",
          "ty": 0,
          "type_option": {
            "0": {
              "data": ""
            }
          },
          "visibility": true,
          "width": 150
        },
        "QfCqmc": {
          "created_at": 1690639671,
          "id": "QfCqmc",
          "is_primary": false,
          "last_modified": 1690639680,
          "name": "Text",
          "ty": 1,
          "type_option": {
            "0": {
              "data": "",
              "format": 0,
              "name": "Number",
              "scale": 0,
              "symbol": "RUB"
            },
            "1": {
              "format": 1,
              "name": "Number",
              "scale": 0,
              "symbol": "RUB"
            }
          },
          "visibility": true,
          "width": 120
        },
        "e-5TiR": {
          "created_at": 1690639659,
          "id": "e-5TiR",
          "is_primary": false,
          "last_modified": 1690639659,
          "name": "Done",
          "ty": 5,
          "type_option": {
            "5": {
              "is_selected": false
            }
          },
          "visibility": true,
          "width": 150
        },
        "vdCF8I": {
          "created_at": 1690639694,
          "id": "vdCF8I",
          "is_primary": false,
          "last_modified": 1690639697,
          "name": "Text",
          "ty": 6,
          "type_option": {
            "0": {
              "content": "",
              "data": "",
              "url": ""
            },
            "6": {
              "content": "",
              "url": ""
            }
          },
          "visibility": true,
          "width": 120
        }
      },
      "id": "c0e69740-49f0-4790-a488-702e2750ba8d",
      "metas": {
        "iid": "b44b2906-4508-4532-ad9e-2cf33ceae304"
      },
      "views": {
        "b44b2906-4508-4532-ad9e-2cf33ceae304": {
          "created_at": 1690639659,
          "database_id": "c0e69740-49f0-4790-a488-702e2750ba8d",
          "field_orders": [
            {
              "id": "E_50ji"
            },
            {
              "id": "8tbGTb"
            },
            {
              "id": "e-5TiR"
            },
            {
              "id": "QfCqmc"
            },
            {
              "id": "vdCF8I"
            },
            {
              "id": "9U02fU"
            }
          ],
          "filters": [
            {
              "condition": 2,
              "content": "",
              "field_id": "E_50ji",
              "id": "OWu470",
              "ty": 0
            }
          ],
          "groups": [],
          "id": "b44b2906-4508-4532-ad9e-2cf33ceae304",
          "layout": 0,
          "layout_settings": {},
          "modified_at": 1690639708,
          "name": "Untitled",
          "row_orders": [
            {
              "height": 60,
              "id": "bbd404d8-1319-4e4d-84fe-1052c57fe3e7"
            },
            {
              "height": 60,
              "id": "bcfe322e-6272-4ed3-a57e-09645ec1073a"
            },
            {
              "height": 60,
              "id": "5d4418d2-621a-4ac5-ad05-e2c6fcc1bc79"
            }
          ],
          "sorts": [
            {
              "condition": 0,
              "field_id": "E_50ji",
              "id": "s:4SJjUs",
              "ty": 0
            }
          ]
        }
      }
    }
  })
}
