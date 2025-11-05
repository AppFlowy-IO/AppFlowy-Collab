use crate::util::{
  create_folder_with_data, create_folder_with_workspace, make_test_view, parse_view_id,
};
use assert_json_diff::assert_json_include;
use collab::entity::uuid_validation::view_id_from_any_string;
use collab::folder::{FolderData, UserId};
use serde_json::json;

#[test]
fn create_favorite_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), view_id_from_any_string("w1"));
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id, vec![]);
  let view_1_id = view_1.id.to_string();
  folder.insert_view(view_1, None, uid.as_i64());

  // Get view_1 from folder
  let view_1 = folder
    .get_view(&parse_view_id(&view_1_id), Some(uid.as_i64()))
    .unwrap();
  assert!(!view_1.is_favorite);
  folder.add_favorite_view_ids(vec![view_1_id.clone()], uid.as_i64());

  // Check if view_1 is favorite
  let view_1 = folder
    .get_view(&parse_view_id(&view_1_id), Some(uid.as_i64()))
    .unwrap();
  assert!(view_1.is_favorite);

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id, vec![]);
  folder.insert_view(view_2, None, uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.collab.transact(), &workspace_id, Some(uid.as_i64()));
  assert_eq!(views.len(), 2);
  assert_eq!(
    views[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string()
  );
  assert!(views[0].is_favorite);

  assert_eq!(
    views[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string()
  );
  assert!(!views[1].is_favorite);

  let favorites = folder.get_my_favorite_sections(Some(uid.as_i64()));
  assert_eq!(favorites.len(), 1);
}

#[test]
fn add_favorite_view_and_then_remove_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), view_id_from_any_string("w1"));
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id, vec![]);
  let view_1_id = view_1.id.to_string();
  folder.insert_view(view_1, None, uid.as_i64());
  folder.add_favorite_view_ids(vec![view_1_id.clone()], uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, Some(uid.as_i64()));
  assert_eq!(views.len(), 1);
  assert_eq!(
    views[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string()
  );
  assert!(views[0].is_favorite);

  folder.delete_favorite_view_ids(vec![view_1_id], uid.as_i64());
  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, Some(uid.as_i64()));
  assert!(!views[0].is_favorite);
}

#[test]
fn create_multiple_user_favorite_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = view_id_from_any_string("w1");
  let folder_test_1 = create_folder_with_workspace(uid_1.clone(), workspace_id);

  let mut folder_1 = folder_test_1.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id, vec![]);
  let view_1_id = view_1.id.to_string();
  folder_1.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id, vec![]);
  let view_2_id = view_2.id.to_string();
  folder_1.insert_view(view_2, None, uid_1.as_i64());

  folder_1.add_favorite_view_ids(vec![view_1_id.clone(), view_2_id.clone()], uid_1.as_i64());
  let favorites = folder_1.get_my_favorite_sections(Some(uid_1.as_i64()));
  assert_eq!(favorites.len(), 2);
  assert_eq!(
    favorites[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string()
  );
  assert_eq!(
    favorites[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string()
  );
  let workspace_uuid_str = workspace_id.to_string();
  let folder_data = folder_1
    .get_folder_data(&workspace_uuid_str, Some(uid_1.as_i64()))
    .unwrap();

  let uid_2 = UserId::from(2);
  let folder_test2 =
    create_folder_with_data(uid_2.clone(), view_id_from_any_string("w1"), folder_data);
  let favorites = folder_test2.get_my_favorite_sections(Some(uid_2.as_i64()));

  // User 2 can't see user 1's favorites
  assert!(favorites.is_empty());
}

#[test]
fn favorite_data_serde_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = view_id_from_any_string("w1");
  let folder_test = create_folder_with_workspace(uid_1.clone(), workspace_id);

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id, vec![]);
  let view_1_id = view_1.id.to_string();
  folder.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id, vec![]);
  let view_2_id = view_2.id.to_string();
  folder.insert_view(view_2, None, uid_1.as_i64());

  folder.add_favorite_view_ids(vec![view_1_id, view_2_id], uid_1.as_i64());
  let workspace_uuid_str = workspace_id.to_string();
  let folder_data = folder
    .get_folder_data(&workspace_uuid_str, Some(uid_1.as_i64()))
    .unwrap();
  let value = serde_json::to_value(&folder_data).unwrap();
  let w1_uuid = workspace_uuid_str.clone();
  let id_1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string();
  let id_2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string();
  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": null,
      "favorites": {
        "1": [
          {
            "id": &id_1_uuid,
          },
          {
            "id": &id_2_uuid,
          },
        ]
      },
      "views": [],
      "workspace": {
        "child_views": {
          "items": []
        },
        "id": &w1_uuid,
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
  let folder_test = create_folder_with_workspace(uid.clone(), view_id_from_any_string("w1"));
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("1", workspace_id, vec![]);
  let view_1_id = view_1.id.to_string();
  folder.insert_view(view_1, None, uid.as_i64());

  // Insert view_2
  let view_2 = make_test_view("2", workspace_id, vec![]);
  let view_2_id = view_2.id.to_string();
  folder.insert_view(view_2, None, uid.as_i64());

  // Add favorites
  folder.add_favorite_view_ids(vec![view_1_id.clone(), view_2_id], uid.as_i64());

  let favorites = folder.get_my_favorite_sections(Some(uid.as_i64()));
  assert_eq!(favorites.len(), 2);
  assert_eq!(
    favorites[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string()
  );
  assert_eq!(
    favorites[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string()
  );

  folder.delete_favorite_view_ids(vec![view_1_id], uid.as_i64());
  let favorites = folder.get_my_favorite_sections(Some(uid.as_i64()));
  assert_eq!(favorites.len(), 1);
  assert_eq!(
    favorites[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string()
  );

  folder.remove_all_my_favorite_sections(uid.as_i64());
  let favorites = folder.get_my_favorite_sections(Some(uid.as_i64()));
  assert_eq!(favorites.len(), 0);
}
