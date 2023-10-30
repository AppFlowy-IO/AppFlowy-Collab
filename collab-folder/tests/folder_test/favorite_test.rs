use collab_folder::{FolderData, UserId};
use serde_json::json;

use assert_json_diff::assert_json_eq;

use crate::util::{
  create_folder_with_data, create_folder_with_workspace, open_folder_with_db,
  unzip_history_folder_db,
};

#[tokio::test]
async fn create_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  folder_test.add_favorites(vec!["1".to_string(), "2".to_string()]);

  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
}

#[tokio::test]
async fn create_multiple_user_favorite_test() {
  let uid_1 = UserId::from(1);
  let folder_test_1 = create_folder_with_workspace(uid_1.clone(), "w1").await;
  folder_test_1.add_favorites(vec!["1".to_string(), "2".to_string()]);
  let favorites = folder_test_1.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
  let folder_data = folder_test_1.get_folder_data().unwrap();

  let uid_2 = UserId::from(2);
  let folder_test2 = create_folder_with_data(uid_2.clone(), "w1", Some(folder_data)).await;
  let favorites = folder_test2.get_all_favorites();

  // User 2 can't see user 1's favorites
  assert!(favorites.is_empty());
}

#[tokio::test]
async fn favorite_data_serde_test() {
  let uid_1 = UserId::from(1);
  let folder_test_1 = create_folder_with_workspace(uid_1.clone(), "w1").await;
  folder_test_1.add_favorites(vec!["1".to_string(), "2".to_string()]);
  let folder_data = folder_test_1.get_folder_data().unwrap();
  let value = serde_json::to_value(&folder_data).unwrap();
  assert_json_eq!(
    value,
    json!({
      "current_view": "",
      "current_workspace_id": "w1",
      "favorites": {
        "1": [
          "1",
          "2"
        ]
      },
      "views": [],
      "workspaces": [
        {
          "child_views": {
            "items": []
          },
          "created_at": 123,
          "id": "w1",
          "name": "My first workspace"
        }
      ]
    })
  );

  assert_eq!(
    folder_data,
    serde_json::from_value::<FolderData>(value).unwrap()
  );
}

#[tokio::test]
async fn delete_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  folder_test.add_favorites(vec!["1".to_string(), "2".to_string()]);

  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");

  folder_test.delete_favorites(vec!["1".to_string()]);
  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 1);
  assert_eq!(favorites[0].id, "2");

  folder_test.remove_all_favorites();
  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 0);
}

const FOLDER_WITHOUT_FAV: &str = "folder_without_fav";

#[tokio::test]
async fn migrate_from_old_version_folder_without_fav_test() {
  let db_path = unzip_history_folder_db(FOLDER_WITHOUT_FAV).unwrap();
  let folder_test = open_folder_with_db(
    221439819971039232.into(),
    "49af3b85-9343-447a-946d-038f63883399",
    db_path,
  )
  .await;
  let folder_data = folder_test.get_folder_data().unwrap();
  let value = serde_json::to_value(folder_data).unwrap();

  assert_json_eq!(
    value,
    json!({
      "current_view": "631584ec-af71-42c3-94f4-89dcfdafb988",
      "current_workspace_id": "49af3b85-9343-447a-946d-038f63883399",
      "views": [
        {
          "children": {
            "items": [
              {
                "id": "631584ec-af71-42c3-94f4-89dcfdafb988"
              }
            ]
          },
          "icon": null,
          "created_at": 1690602073,
          "desc": "",
          "id": "5cf7eff5-954d-424d-a5e7-032527929019",
          "is_favorite": false,
          "layout": 0,
          "name": "⭐️ Getting started",
          "parent_view_id": "49af3b85-9343-447a-946d-038f63883399"
        },
        {
          "children": {
            "items": []
          },
          "icon": null,
          "created_at": 1690602073,
          "desc": "",
          "id": "631584ec-af71-42c3-94f4-89dcfdafb988",
          "is_favorite": false,
          "layout": 0,
          "name": "Read me",
          "parent_view_id": "5cf7eff5-954d-424d-a5e7-032527929019"
        }
      ],
      "workspaces": [
        {
          "child_views": {
            "items": [
              {
                "id": "5cf7eff5-954d-424d-a5e7-032527929019"
              }
            ]
          },
          "created_at": 1690602073,
          "id": "49af3b85-9343-447a-946d-038f63883399",
          "name": "Workspace"
        }
      ],
      "favorites": {}
    })
  );
}

#[tokio::test]
async fn deserialize_folder_data_without_fav_test() {
  let folder_test = create_folder_with_data(1.into(), "1", Some(folder_data_without_fav())).await;
  let folder_data = folder_test.get_folder_data().unwrap();
  let value = serde_json::to_value(folder_data).unwrap();
  assert_json_eq!(
    value,
    json!({
      "current_view": "",
      "current_workspace_id": "w1",
      "views": [
        {
          "children": {
            "items": [
              {
                "id": "1_1"
              },
              {
                "id": "1_2"
              },
              {
                "id": "1_3"
              }
            ]
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "w1"
        },
        {
          "children": {
            "items": []
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1_1",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1"
        },
        {
          "children": {
            "items": [
              {
                "id": "1_2_1"
              },
              {
                "id": "1_2_2"
              }
            ]
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1_2",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1"
        },
        {
          "children": {
            "items": []
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1_2_1",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1_2"
        },
        {
          "children": {
            "items": []
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1_2_2",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1_2"
        },
        {
          "children": {
            "items": []
          },
          "icon": null,
          "created_at": 0,
          "desc": "",
          "id": "1_3",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1"
        }
      ],
      "workspaces": [
        {
          "child_views": {
            "items": [
              {
                "id": "1"
              }
            ]
          },
          "created_at": 123,
          "id": "w1",
          "name": "My first workspace"
        }
      ],
      "favorites": {}
    })
  )
}

fn folder_data_without_fav() -> FolderData {
  let json = json!({
    "current_view": "",
    "current_workspace_id": "w1",
    "views": [
      {
        "children": {
          "items": [
            {
              "id": "1_1"
            },
            {
              "id": "1_2"
            },
            {
              "id": "1_3"
            }
          ]
        },
        "icon": null,
        "created_at": 0,
        "desc": "",
        "id": "1",
        "layout": 0,
        "name": "",
        "parent_view_id": "w1"
      },
      {
        "children": {
          "items": []
        },
        "icon": null,
        "created_at": 0,
        "desc": "",
        "id": "1_1",
        "layout": 0,
        "name": "",
        "parent_view_id": "1"
      },
      {
        "children": {
          "items": [
            {
              "id": "1_2_1"
            },
            {
              "id": "1_2_2"
            }
          ]
        },
        "icon": null,
        "created_at": 0,
        "desc": "",
        "id": "1_2",
        "layout": 0,
        "name": "",
        "parent_view_id": "1"
      },
      {
        "children": {
          "items": []
        },
        "created_at": 0,
        "desc": "",
        "icon": null,
        "id": "1_2_1",
        "layout": 0,
        "name": "",
        "parent_view_id": "1_2"
      },
      {
        "children": {
          "items": []
        },
        "created_at": 0,
        "desc": "",
        "icon": null,
        "id": "1_2_2",
        "layout": 0,
        "name": "",
        "parent_view_id": "1_2"
      },
      {
        "children": {
          "items": []
        },
        "created_at": 0,
        "desc": "",
        "icon": null,
        "id": "1_3",
        "layout": 0,
        "name": "",
        "parent_view_id": "1"
      }
    ],
    "workspaces": [
      {
        "child_views": {
          "items": [
            {
              "id": "1"
            }
          ]
        },
        "created_at": 123,
        "id": "w1",
        "name": "My first workspace"
      }
    ]
  });
  serde_json::from_value::<FolderData>(json).unwrap()
}
