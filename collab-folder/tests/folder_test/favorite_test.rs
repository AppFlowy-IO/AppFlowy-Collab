use crate::util::{create_folder_with_data, create_folder_with_workspace, make_test_view};
use assert_json_diff::assert_json_include;
use collab_folder::{FolderData, UserId};
use serde_json::json;

#[test]
fn create_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None, uid.as_i64());

  // Get view_1 from folder
  let view_1 = folder.get_view("1", uid.as_i64()).unwrap();
  assert!(!view_1.is_favorite);
  folder.add_favorite_view_ids(vec!["1".to_string()], uid.as_i64());

  // Check if view_1 is favorite
  let view_1 = folder.get_view("1", uid.as_i64()).unwrap();
  assert!(view_1.is_favorite);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.collab.transact(), &workspace_id, uid.as_i64());
  assert_eq!(views.len(), 2);
  assert_eq!(views[0].id, "1");
  assert!(views[0].is_favorite);

  assert_eq!(views[1].id, "2");
  assert!(!views[1].is_favorite);

  let favorites = folder.get_my_favorite_sections(uid.as_i64());
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
  folder.insert_view(view_1, None, uid.as_i64());
  folder.add_favorite_view_ids(vec!["1".to_string()], uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, uid.as_i64());
  assert_eq!(views.len(), 1);
  assert_eq!(views[0].id, "1");
  assert!(views[0].is_favorite);

  folder.delete_favorite_view_ids(vec!["1".to_string()], uid.as_i64());
  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, uid.as_i64());
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
  folder_1.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder_1.insert_view(view_2, None, uid_1.as_i64());

  folder_1.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()], uid_1.as_i64());
  let favorites = folder_1.get_my_favorite_sections(uid_1.as_i64());
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
  let folder_data = folder_1
    .get_folder_data(&workspace_id, uid_1.as_i64())
    .unwrap();

  let uid_2 = UserId::from(2);
  let folder_test2 = create_folder_with_data(uid_2.clone(), "w1", folder_data);
  let favorites = folder_test2.get_my_favorite_sections(uid_2.as_i64());

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
  folder.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid_1.as_i64());

  folder.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()], uid_1.as_i64());
  let folder_data = folder
    .get_folder_data(&workspace_id, uid_1.as_i64())
    .unwrap();
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
  folder.insert_view(view_1, None, uid.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid.as_i64());

  // Add favorites
  folder.add_favorite_view_ids(vec!["1".to_string(), "2".to_string()], uid.as_i64());

  let favorites = folder.get_my_favorite_sections(uid.as_i64());
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");

  folder.delete_favorite_view_ids(vec!["1".to_string()], uid.as_i64());
  let favorites = folder.get_my_favorite_sections(uid.as_i64());
  assert_eq!(favorites.len(), 1);
  assert_eq!(favorites[0].id, "2");

  folder.remove_all_my_favorite_sections(uid.as_i64());
  let favorites = folder.get_my_favorite_sections(uid.as_i64());
  assert_eq!(favorites.len(), 0);
}
