use assert_json_diff::assert_json_include;
use collab_folder::{UserId, timestamp};
use serde_json::json;

use crate::util::{create_folder_with_workspace, make_test_view};

#[test]
fn create_child_views_test() {
  let uid = UserId::from(1);
  let workspace_id = "fake_w_1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);
  let v_1_1 = make_test_view("1_1", "1", vec![]);
  let v_1_2 = make_test_view("1_2", "1", vec![]);
  let v_1_2_1 = make_test_view("1_2_1", "1_2", vec![]);
  let v_1_2_2 = make_test_view("1_2_2", "1_2", vec![]);
  let v_1_3 = make_test_view("1_3", "1", vec![]);
  let v_1 = make_test_view("1", &workspace_id, vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let time = timestamp();
  folder
    .body
    .views
    .insert(&mut txn, v_1.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_2.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_2_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_2_2, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_3, None, uid.as_i64());

  let v_1_child_views = folder
    .body
    .views
    .get_views_belong_to(&txn, &v_1.id, uid.as_i64());
  assert_eq!(v_1_child_views.len(), 3);

  let v_1_2_child_views = folder
    .body
    .views
    .get_views_belong_to(&txn, &v_1_2.id, uid.as_i64());
  assert_eq!(v_1_2_child_views.len(), 2);

  let folder_data = folder
    .body
    .get_folder_data(&txn, &workspace_id, uid.as_i64())
    .unwrap();
  let value = serde_json::to_value(folder_data).unwrap();
  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": "",
      "favorites": {},
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
          "created_at": time,
          "icon": null,
          "id": "1",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "fake_w_1"
        },
        {
          "children": {
            "items": []
          },
          "created_at": time,
          "icon": null,
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
          "created_at": time,
          "icon": null,
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
          "created_at": time,
          "icon": null,
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
          "created_at": time,
          "icon": null,
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
          "created_at": time,
          "icon": null,
          "id": "1_3",
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": "1"
        }
      ],
      "workspace": {
        "child_views": {
          "items": [
            {
              "id": "1"
            }
          ]
        },
        "id": "fake_w_1",
        "name": ""
      }
    })
  );
}

#[test]
fn move_child_views_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let v_1_1 = make_test_view("1_1", "1", vec![]);
  let v_1_2 = make_test_view("1_2", "1", vec![]);
  let v_1_3 = make_test_view("1_3", "1", vec![]);
  let v_1 = make_test_view(
    "1",
    "w1",
    vec!["1_1".to_string(), "1_2".to_string(), "1_3".to_string()],
  );

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, v_1.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_2, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, v_1_3, None, uid.as_i64());

  let v_1_child_views = folder
    .body
    .views
    .get_views_belong_to(&txn, &v_1.id, uid.as_i64());
  assert_eq!(v_1_child_views[0].id, "1_1");
  assert_eq!(v_1_child_views[1].id, "1_2");
  assert_eq!(v_1_child_views[2].id, "1_3");

  folder.body.views.move_child(&mut txn, &v_1.id, 2, 0);
  folder.body.views.move_child(&mut txn, &v_1.id, 0, 1);

  let v_1_child_views = folder
    .body
    .views
    .get_view(&txn, &v_1.id, uid.as_i64())
    .unwrap();
  assert_eq!(v_1_child_views.children[0].id, "1_1");
  assert_eq!(v_1_child_views.children[1].id, "1_3");
  assert_eq!(v_1_child_views.children[2].id, "1_2");
}

#[test]
fn delete_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let view_1 = make_test_view("1_1", "w1", vec![]);
  let view_2 = make_test_view("1_2", "w1", vec![]);
  let view_3 = make_test_view("1_3", "w1", vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, view_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_3, None, uid.as_i64());

  folder.body.views.remove_child(&mut txn, "w1", 1);
  let w_1_child_views = folder
    .body
    .views
    .get_views_belong_to(&txn, "w1", uid.as_i64());
  assert_eq!(w_1_child_views[0].id, "1_1");
  assert_eq!(w_1_child_views[1].id, "1_3");
}

#[test]
fn delete_child_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_1_1 = make_test_view("v1_1", "v1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, view_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_1_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2, None, uid.as_i64());

  let views = folder
    .body
    .views
    .get_views_belong_to(&txn, "v1", uid.as_i64());
  assert_eq!(views.len(), 1);

  folder
    .body
    .views
    .delete_views(&mut txn, vec!["v1_1".to_string()]);
  let views = folder
    .body
    .views
    .get_views_belong_to(&txn, "v1", uid.as_i64());
  assert!(views.is_empty());
}

#[test]
fn create_orphan_child_views_test() {
  let uid = UserId::from(1);
  let workspace_id = "fake_w_1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);
  let view_1 = make_test_view("1", &workspace_id, vec![]);

  // The orphan view: the parent_view_id equal to the view_id
  let view_2 = make_test_view("2", "2", vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, view_1.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2.clone(), None, uid.as_i64());

  let child_views = folder
    .body
    .views
    .get_views_belong_to(&txn, &workspace_id, uid.as_i64());
  assert_eq!(child_views.len(), 1);

  let orphan_views = folder
    .body
    .views
    .get_orphan_views_with_txn(&txn, uid.as_i64());
  assert_eq!(orphan_views.len(), 1);

  // The folder data should contains the orphan view
  let folder_data = folder
    .body
    .get_folder_data(&txn, &workspace_id, uid.as_i64())
    .unwrap();
  assert_json_include!(
    actual: json!(folder_data),
    expected: json!({
          "current_view": "",
          "favorites": {},
          "recent": {},
          "trash": {},
          "views": [
            {
              "children": {
                "items": []
              },
              "created_by": 1,
              "icon": null,
              "id": "1",
              "is_favorite": false,
              "last_edited_by": 1,
              "layout": 0,
              "name": "",
              "parent_view_id": "fake_w_1"
            },
            {
              "children": {
                "items": []
              },
              "created_by": 1,
              "icon": null,
              "id": "2",
              "is_favorite": false,
              "last_edited_by": 1,
              "layout": 0,
              "name": "",
              "parent_view_id": "2"
            }
          ],
          "workspace": {
            "child_views": {
              "items": [
                {
                  "id": "1"
                }
              ]
            },
            "created_by": 1,
            "id": "fake_w_1",
            "last_edited_by": 1,
            "name": ""
          }
        })
  );
}
