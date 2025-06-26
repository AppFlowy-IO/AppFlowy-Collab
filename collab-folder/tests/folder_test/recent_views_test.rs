use assert_json_diff::assert_json_include;
use collab_folder::{FolderData, Section, UserId, timestamp};
use serde_json::json;

use crate::util::{create_folder_with_data, create_folder_with_workspace, make_test_view};

#[test]
fn create_recent_views_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let id_1 = "view_1";

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view(id_1, workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None, uid.as_i64());

  // Get view_1 from folder
  let view_1 = folder.get_view(id_1, uid.as_i64()).unwrap();
  // Check if view_1 has been added into recent section.
  assert!(!folder.is_view_in_section(Section::Recent, &view_1.id, uid.as_i64()));
  folder.add_recent_view_ids(vec![id_1.to_string()], uid.as_i64());

  let view_1 = folder.get_view(id_1, uid.as_i64()).unwrap();
  assert!(folder.is_view_in_section(Section::Recent, &view_1.id, uid.as_i64()));

  let id_2: &str = "view_2";

  // Insert view_2
  let view_2 = make_test_view(id_2, workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, uid.as_i64());
  assert_eq!(views.len(), 2);
  assert_eq!(views[0].id, id_1);
  assert_eq!(views[1].id, id_2);

  let recent = folder.get_my_recent_sections(uid.as_i64());
  assert_eq!(recent.len(), 1);
}

#[test]
fn add_view_into_recent_and_then_remove_it_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let id_1 = "view_1";

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view(id_1, workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None, uid.as_i64());
  folder.add_recent_view_ids(vec![id_1.to_string()], uid.as_i64());

  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, uid.as_i64());
  assert_eq!(views.len(), 1);
  assert_eq!(views[0].id, id_1);
  // in recent section
  assert!(folder.is_view_in_section(Section::Recent, &views[0].id, uid.as_i64()));

  folder.delete_recent_view_ids(vec![id_1.to_string()], uid.as_i64());
  let views =
    folder
      .body
      .views
      .get_views_belong_to(&folder.transact(), &workspace_id, uid.as_i64());
  // not in recent section
  assert!(!folder.is_view_in_section(Section::Recent, &views[0].id, uid.as_i64()));
}

#[test]
fn create_multiple_user_recent_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test_1 = create_folder_with_workspace(uid_1.clone(), &workspace_id);

  let mut folder_1 = folder_test_1.folder;

  // Insert view_1
  let id_1 = "view_1";
  let view_1 = make_test_view(id_1, workspace_id.as_str(), vec![]);
  folder_1.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let id_2 = "view_2";
  let view_2 = make_test_view(id_2, workspace_id.as_str(), vec![]);
  folder_1.insert_view(view_2, None, uid_1.as_i64());

  folder_1.add_recent_view_ids(vec![id_1.to_string(), id_2.to_string()], uid_1.as_i64());
  let recent = folder_1.get_my_recent_sections(uid_1.as_i64());
  assert_eq!(recent.len(), 2);
  assert_eq!(recent[0].id, id_1);
  assert_eq!(recent[1].id, id_2);
  let folder_data = folder_1
    .get_folder_data(&workspace_id, uid_1.as_i64())
    .unwrap();

  let uid_2 = UserId::from(2);
  let folder_test2 = create_folder_with_data(uid_2.clone(), "w1", folder_data);
  let recent = folder_test2.get_my_recent_sections(uid_2.as_i64());

  // User 2 can't see user 1's recent views
  assert!(recent.is_empty());
}

#[test]
fn recent_data_serde_test() {
  let uid_1 = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid_1.clone(), &workspace_id);

  let mut folder = folder_test.folder;

  // Insert view_1
  let view_1 = make_test_view("view_1", workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None, uid_1.as_i64());

  // Insert view_2
  let view_2 = make_test_view("view_2", workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid_1.as_i64());

  let time = timestamp();
  folder.add_recent_view_ids(
    vec!["view_1".to_string(), "view_2".to_string()],
    uid_1.as_i64(),
  );
  let folder_data = folder
    .get_folder_data(&workspace_id, uid_1.as_i64())
    .unwrap();
  let value = serde_json::to_value(&folder_data).unwrap();
  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": "",
      "recent": {
        "1": [
          {
            "id": "view_1",
            "timestamp": time
          },
          {
            "id": "view_2",
            "timestamp": time
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
fn delete_recent_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  // Insert view_1
  let id_1 = "view_1";
  let view_1 = make_test_view(id_1, workspace_id.as_str(), vec![]);
  folder.insert_view(view_1, None, uid.as_i64());

  // Insert view_2
  let id_2 = "view_2";
  let view_2 = make_test_view(id_2, workspace_id.as_str(), vec![]);
  folder.insert_view(view_2, None, uid.as_i64());

  folder.add_recent_view_ids(vec![id_1.to_string(), id_2.to_string()], uid.as_i64());
  let recent = folder.get_my_recent_sections(uid.as_i64());
  assert_eq!(recent.len(), 2);
  assert_eq!(recent[0].id, id_1);
  assert_eq!(recent[1].id, id_2);

  folder.remove_all_my_recent_sections(uid.as_i64());
  let recent = folder.get_my_recent_sections(uid.as_i64());
  assert_eq!(recent.len(), 0);
}
