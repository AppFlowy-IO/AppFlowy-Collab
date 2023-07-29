use assert_json_diff::assert_json_eq;
use collab_folder::core::FolderData;
use serde_json::json;

use crate::util::{
  create_folder_with_data, create_folder_with_workspace, open_folder_with_db,
  unzip_history_folder_db,
};

#[tokio::test]
async fn create_favorite_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  folder_test.add_favorites(vec!["1".to_string(), "2".to_string()]);

  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
}
#[tokio::test]
async fn delete_favorite_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
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
    221439819971039232,
    "49af3b85-9343-447a-946d-038f63883399",
    db_path,
  );
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
          "cover_url": null,
          "created_at": 1690602073,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 1690602073,
          "desc": "",
          "icon_url": null,
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
      ]
    })
  );
}

#[tokio::test]
async fn deserialize_folder_data_without_fav_test() {
  let folder_test = create_folder_with_data("1", Some(folder_data_without_fav()));
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
          "cover_url": null,
          "created_at": 0,
          "desc": "",
          "icon_url": null,
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
      ]
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
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
        "id": "1",
        "layout": 0,
        "name": "",
        "parent_view_id": "w1"
      },
      {
        "children": {
          "items": []
        },
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
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
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
        "id": "1_2",
        "layout": 0,
        "name": "",
        "parent_view_id": "1"
      },
      {
        "children": {
          "items": []
        },
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
        "id": "1_2_1",
        "layout": 0,
        "name": "",
        "parent_view_id": "1_2"
      },
      {
        "children": {
          "items": []
        },
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
        "id": "1_2_2",
        "layout": 0,
        "name": "",
        "parent_view_id": "1_2"
      },
      {
        "children": {
          "items": []
        },
        "cover_url": null,
        "created_at": 0,
        "desc": "",
        "icon_url": null,
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
