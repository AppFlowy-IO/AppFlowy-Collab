use assert_json_diff::assert_json_include;
use collab_folder::{timestamp, UserId};
use serde_json::json;

use crate::util::{create_folder_with_workspace, make_test_view};

#[tokio::test]
async fn create_child_views_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "fake_w_1").await;
  let workspace_id = folder_test.get_workspace_id();
  let view_1_1 = make_test_view("1_1", "1", vec![]);
  let view_1_2 = make_test_view("1_2", "1", vec![]);
  let view_1_2_1 = make_test_view("1_2_1", "1_2", vec![]);
  let view_1_2_2 = make_test_view("1_2_2", "1_2", vec![]);
  let view_1_3 = make_test_view("1_3", "1", vec![]);
  let view_1 = make_test_view("1", &workspace_id, vec![]);

  let time = timestamp();
  folder_test.insert_view(view_1.clone(), None);
  folder_test.insert_view(view_1_1, None);
  folder_test.insert_view(view_1_2.clone(), None);
  folder_test.insert_view(view_1_2_1, None);
  folder_test.insert_view(view_1_2_2, None);
  folder_test.insert_view(view_1_3, None);

  let v_1_child_views = folder_test.views.get_views_belong_to(&view_1.id);
  assert_eq!(v_1_child_views.len(), 3);

  let v_1_2_child_views = folder_test.views.get_views_belong_to(&view_1_2.id);
  assert_eq!(v_1_2_child_views.len(), 2);

  let folder_data = folder_test.get_folder_data().unwrap();
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
          "desc": "",
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
          "desc": "",
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
          "desc": "",
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
          "desc": "",
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
          "desc": "",
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
          "desc": "",
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

#[tokio::test]
async fn move_child_views_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1_1 = make_test_view("1_1", "1", vec![]);
  let view_1_2 = make_test_view("1_2", "1", vec![]);
  let view_1_3 = make_test_view("1_3", "1", vec![]);
  let view_1 = make_test_view(
    "1",
    "w1",
    vec!["1_1".to_string(), "1_2".to_string(), "1_3".to_string()],
  );

  folder_test.insert_view(view_1.clone(), None);
  folder_test.insert_view(view_1_1, None);
  folder_test.insert_view(view_1_2, None);
  folder_test.insert_view(view_1_3, None);

  let v_1_child_views = folder_test.views.get_views_belong_to(&view_1.id);
  assert_eq!(v_1_child_views[0].id, "1_1");
  assert_eq!(v_1_child_views[1].id, "1_2");
  assert_eq!(v_1_child_views[2].id, "1_3");

  folder_test.views.move_child(&view_1.id, 2, 0);
  folder_test.views.move_child(&view_1.id, 0, 1);

  let v_1_child_views = folder_test.views.get_view(&view_1.id).unwrap();
  assert_eq!(v_1_child_views.children[0].id, "1_1");
  assert_eq!(v_1_child_views.children[1].id, "1_3");
  assert_eq!(v_1_child_views.children[2].id, "1_2");
}

#[tokio::test]
async fn delete_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1 = make_test_view("1_1", "w1", vec![]);
  let view_2 = make_test_view("1_2", "w1", vec![]);
  let view_3 = make_test_view("1_3", "w1", vec![]);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);
  folder_test.insert_view(view_3, None);

  folder_test.views.remove_child("w1", 1);
  let w_1_child_views = folder_test.views.get_views_belong_to("w1");
  assert_eq!(w_1_child_views[0].id, "1_1");
  assert_eq!(w_1_child_views[1].id, "1_3");
}

#[tokio::test]
async fn delete_child_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_1_1 = make_test_view("v1_1", "v1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_1_1, None);
  folder_test.insert_view(view_2, None);

  let views = folder_test.views.get_views_belong_to("v1");
  assert_eq!(views.len(), 1);

  folder_test.views.delete_views(vec!["v1_1".to_string()]);
  let views = folder_test.views.get_views_belong_to("v1");
  assert!(views.is_empty());
}

#[tokio::test]
async fn create_orphan_child_views_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "fake_w_1").await;
  let workspace_id = folder_test.get_workspace_id();
  let view_1 = make_test_view("1", "fake_w_1", vec![]);

  // The orphan view: the parent_view_id equal to the view_id
  let view_2 = make_test_view("2", "2", vec![]);

  folder_test.insert_view(view_1.clone(), None);
  folder_test.insert_view(view_2.clone(), None);

  let child_views = folder_test.views.get_views_belong_to(&workspace_id);
  assert_eq!(child_views.len(), 1);

  let orphan_views = folder_test.views.get_orphan_views();
  assert_eq!(orphan_views.len(), 1);

  // The folder data should contains the orphan view
  let folder_data = folder_test.get_folder_data().unwrap();
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
              "desc": "",
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
              "desc": "",
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
