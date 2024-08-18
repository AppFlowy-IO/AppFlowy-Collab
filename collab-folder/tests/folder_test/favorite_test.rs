use assert_json_diff::assert_json_include;
use collab_folder::{FolderData, UserId};
use serde_json::json;

use crate::util::{
  create_folder_with_data, create_folder_with_workspace, make_test_view, open_folder_with_db,
  unzip_history_folder_db,
};

#[test]
fn create_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None);

  // Get view_1 from folder
  let view_1 = folder.get_view("1").unwrap();
  assert!(!view_1.is_favorite);
  folder.add_favorite_view_ids(vec!["1".to_string()]);

  // Check if view_1 is favorite
  let view_1 = folder.get_view("1").unwrap();
  assert!(view_1.is_favorite);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None);

  let views = folder
    .body
    .views
    .get_views_belong_to(&folder.collab.transact(), &workspace_id);
  assert_eq!(views.len(), 2);
  assert_eq!(views[0].id, "1");
  assert!(views[0].is_favorite);

  assert_eq!(views[1].id, "2");
  assert!(!views[1].is_favorite);

  let favorites = folder.get_my_favorite_sections();
  assert_eq!(favorites.len(), 1);
}

#[test]
fn add_favorite_view_and_then_remove_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None);
  folder.add_favorite_view_ids(vec!["1".to_string()]);

  let views = folder
    .body
    .views
    .get_views_belong_to(&folder.transact(), &workspace_id);
  assert_eq!(views.len(), 1);
  assert_eq!(views[0].id, "1");
  assert!(views[0].is_favorite);

  folder.delete_favorite_view_ids(vec!["1".to_string()]);
  let views = folder
    .body
    .views
    .get_views_belong_to(&folder.transact(), &workspace_id);
  assert!(!views[0].is_favorite);
}

#[test]
fn create_multiple_user_favorite_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test_1 = create_folder_with_workspace(uid_1.clone(), &workspace_id);

  let mut folder_1 = folder_test_1.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder_1.insert_view(view_1, None);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder_1.insert_view(view_2, None);

  folder_1.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()]);
  let favorites = folder_1.get_my_favorite_sections();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
  let folder_data = folder_1.get_folder_data(&workspace_id).unwrap();

  let uid_2 = UserId::from(2);
  let folder_test2 = create_folder_with_data(uid_2.clone(), "w1", folder_data);
  let favorites = folder_test2.get_my_favorite_sections();

  // User 2 can't see user 1's favorites
  assert!(favorites.is_empty());
}

#[test]
fn favorite_data_serde_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid_1.clone(), &workspace_id);

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None);

  folder.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()]);
  let folder_data = folder.get_folder_data(&workspace_id).unwrap();
  let value = serde_json::to_value(&folder_data).unwrap();
  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": "",
      "favorites": {
        "1": [
          {
            "id": "1",
          },
          {
            "id": "2",
          },
        ]
      },
      "views": [],
      "workspace": {
        "child_views": {
          "items": []
        },
        "id": "w1",
        "name": ""
      }
    })
  );

  assert_eq!(
    folder_data,
    serde_json::from_value::<FolderData>(value).unwrap()
  );
}

#[test]
fn delete_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None);

  folder.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()]);
  let favorites = folder.get_my_favorite_sections();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");

  folder.delete_favorite_view_ids(vec!["1".to_string()]);
  let favorites = folder.get_my_favorite_sections();
  assert_eq!(favorites.len(), 1);
  assert_eq!(favorites[0].id, "2");

  folder.remove_all_my_favorite_sections();
  let favorites = folder.get_my_favorite_sections();
  assert_eq!(favorites.len(), 0);
}

const FOLDER_WITHOUT_FAV: &str = "folder_without_fav";
const FOLDER_WITH_FAV_V1: &str = "folder_with_fav_v1";

#[test]
fn migrate_from_old_version_folder_without_fav_test() {
  let (_cleaner, db_path) = unzip_history_folder_db(FOLDER_WITHOUT_FAV).unwrap();
  let folder_test = open_folder_with_db(
    221439819971039232.into(),
    "49af3b85-9343-447a-946d-038f63883399",
    db_path,
  );
  let mut folder = folder_test.folder;
  {
    let mut txn = folder.collab.transact_mut();
    folder.body.migrate_workspace_to_view(&mut txn);
  }
  let workspace_id = folder.get_workspace_id().unwrap();

  let folder_data = folder.get_folder_data(&workspace_id).unwrap();
  let value = serde_json::to_value(folder_data).unwrap();

  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": "631584ec-af71-42c3-94f4-89dcfdafb988",
      "favorites": {},
      "views": [
        {
          "children": {
            "items": [
              {
                "id": "631584ec-af71-42c3-94f4-89dcfdafb988"
              }
            ]
          },
          "created_at": 1690602073,
          "desc": "",
          "icon": null,
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
          "created_at": 1690602073,
          "desc": "",
          "icon": null,
          "id": "631584ec-af71-42c3-94f4-89dcfdafb988",
          "is_favorite": false,
          "layout": 0,
          "name": "Read me",
          "parent_view_id": "5cf7eff5-954d-424d-a5e7-032527929019"
        }
      ],
      "workspace": {
        "child_views": {
          "items": [
            {
              "id": "5cf7eff5-954d-424d-a5e7-032527929019"
            }
          ]
        },
        "id": "49af3b85-9343-447a-946d-038f63883399",
        "name": "Workspace"
      }
    })
  );
}

#[test]
fn migrate_favorite_v1_test() {
  let (_cleaner, db_path) = unzip_history_folder_db(FOLDER_WITH_FAV_V1).unwrap();
  let folder_test = open_folder_with_db(
    254954554859196416.into(),
    "835f64ab-9efc-4365-8055-1e66ee03c555",
    db_path,
  );
  let mut folder = folder_test.folder;
  let workspace_id = folder.get_workspace_id().unwrap();

  // Migrate the favorites from v1 to v2
  let favorites = folder.get_favorite_v1();
  assert_eq!(favorites.len(), 2);

  folder.add_favorite_view_ids(favorites.into_iter().map(|fav| fav.id).collect::<Vec<_>>());
  folder
    .body
    .migrate_workspace_to_view(&mut folder.collab.transact_mut());

  let folder_data = folder.get_folder_data(&workspace_id).unwrap();
  let value = serde_json::to_value(folder_data).unwrap();
  assert_json_include!(
    actual: value,
    expected: json!( {
      "current_view": "9330d783-d10d-4a15-84d3-1fa4fa2e8cc4",
      "favorites": {
        "254954554859196416": [
          {
            "id": "36e0a35e-c636-48d6-9e50-e2e2ee8a1d9f"
          },
          {
            "id": "9330d783-d10d-4a15-84d3-1fa4fa2e8cc4"
          }
        ]
      },
      "views": [
        {
          "children": {
            "items": [
              {
                "id": "36e0a35e-c636-48d6-9e50-e2e2ee8a1d9f"
              },
              {
                "id": "9330d783-d10d-4a15-84d3-1fa4fa2e8cc4"
              },
              {
                "id": "c96d9587-0f6a-4d6b-8d59-6d72f5dcaa4e"
              }
            ]
          },
          "created_at": 1698592608,
          "desc": "",
          "icon": null,
          "id": "ddf06dcf-1a01-4d0d-b973-9d6a892f68b5",
          "is_favorite": false,
          "layout": 0,
          "name": "⭐️ Getting started",
          "parent_view_id": "835f64ab-9efc-4365-8055-1e66ee03c555"
        },
        {
          "children": {
            "items": []
          },
          "created_at": 1698661285,
          "desc": "",
          "icon": null,
          "id": "36e0a35e-c636-48d6-9e50-e2e2ee8a1d9f",
          "is_favorite": true,
          "layout": 1,
          "name": "database 1",
          "parent_view_id": "ddf06dcf-1a01-4d0d-b973-9d6a892f68b5"
        },
        {
          "children": {
            "items": []
          },
          "created_at": 1698661296,
          "desc": "",
          "icon": null,
          "id": "9330d783-d10d-4a15-84d3-1fa4fa2e8cc4",
          "is_favorite": true,
          "layout": 0,
          "name": "document 1",
          "parent_view_id": "ddf06dcf-1a01-4d0d-b973-9d6a892f68b5"
        },
        {
          "children": {
            "items": []
          },
          "created_at": 1698661316,
          "desc": "",
          "icon": null,
          "id": "c96d9587-0f6a-4d6b-8d59-6d72f5dcaa4e",
          "is_favorite": false,
          "layout": 1,
          "name": "Untitled",
          "parent_view_id": "ddf06dcf-1a01-4d0d-b973-9d6a892f68b5"
        }
      ],
      "workspace": {
        "child_views": {
          "items": [
            {
              "id": "ddf06dcf-1a01-4d0d-b973-9d6a892f68b5"
            }
          ]
        },
        "id": "835f64ab-9efc-4365-8055-1e66ee03c555",
        "name": "Workspace"
      }
    })
  );
}
