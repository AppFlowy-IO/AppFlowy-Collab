use serde_json::json;

use assert_json_diff::assert_json_eq;

use crate::util::{create_folder_with_workspace, make_test_view};

#[tokio::test]
async fn create_child_views_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let view_1_1 = make_test_view("1_1", "1", vec![]);
  let view_1_2 = make_test_view("1_2", "1", vec![]);
  let view_1_2_1 = make_test_view("1_2_1", "1_2", vec![]);
  let view_1_2_2 = make_test_view("1_2_2", "1_2", vec![]);
  let view_1_3 = make_test_view("1_3", "1", vec![]);
  let view_1 = make_test_view("1", "w1", vec![]);

  folder_test.insert_view(view_1.clone());
  folder_test.insert_view(view_1_1);
  folder_test.insert_view(view_1_2.clone());
  folder_test.insert_view(view_1_2_1);
  folder_test.insert_view(view_1_2_2);
  folder_test.insert_view(view_1_3);

  let v_1_child_views = folder_test.views.get_views_belong_to(&view_1.id);
  assert_eq!(v_1_child_views.len(), 3);

  let v_1_2_child_views = folder_test.views.get_views_belong_to(&view_1_2.id);
  assert_eq!(v_1_2_child_views.len(), 2);

  let folder_data = folder_test.get_folder_data().unwrap();
  let value = serde_json::to_value(&folder_data).unwrap();
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
    })
  );
}

#[tokio::test]
async fn move_child_views_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let view_1_1 = make_test_view("1_1", "1", vec![]);
  let view_1_2 = make_test_view("1_2", "1", vec![]);
  let view_1_3 = make_test_view("1_3", "1", vec![]);
  let view_1 = make_test_view(
    "1",
    "w1",
    vec!["1_1".to_string(), "1_2".to_string(), "1_3".to_string()],
  );

  folder_test.insert_view(view_1.clone());
  folder_test.insert_view(view_1_1);
  folder_test.insert_view(view_1_2);
  folder_test.insert_view(view_1_3);

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
  let folder_test = create_folder_with_workspace("1", "w1");
  let view_1 = make_test_view("1_1", "w1", vec![]);
  let view_2 = make_test_view("1_2", "w1", vec![]);
  let view_3 = make_test_view("1_3", "w1", vec![]);
  folder_test.insert_view(view_1);
  folder_test.insert_view(view_2);
  folder_test.insert_view(view_3);

  folder_test.views.remove_child("w1", 1);
  let w_1_child_views = folder_test.views.get_views_belong_to("w1");
  assert_eq!(w_1_child_views[0].id, "1_1");
  assert_eq!(w_1_child_views[1].id, "1_3");
}

#[tokio::test]
async fn delete_child_view_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_1_1 = make_test_view("v1_1", "v1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  folder_test.insert_view(view_1);
  folder_test.insert_view(view_1_1);
  folder_test.insert_view(view_2);

  let views = folder_test.views.get_views_belong_to("v1");
  assert_eq!(views.len(), 1);

  folder_test.views.delete_views(vec!["v1_1".to_string()]);
  let views = folder_test.views.get_views_belong_to("v1");
  assert!(views.is_empty());
}
