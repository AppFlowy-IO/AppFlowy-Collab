use collab_folder::{timestamp, UserId};
use serde_json::json;

use crate::util::{create_folder, make_test_view};

#[tokio::test]
async fn folder_json_serde() {
  let folder_test = create_folder(1.into(), "fake_w_1").await;
  let time = timestamp();
  assert_json_diff::assert_json_include!(
    actual: folder_test.to_json_value(),
    expected: json!( {
          "meta": {
            "current_view": "",
            "current_workspace": "fake_w_1"
          },
          "relation": {
            "fake_w_1": []
          },
          "section": {
            "favorite": {}
          },
          "trash": [],
          "views": {
            "fake_w_1": {
              "bid": "",
              "created_at": time,
              "desc": "",
              "icon": "",
              "id": "fake_w_1",
              "layout": 0,
              "name": ""
            }
          }
        }),
  );
}

#[tokio::test]
async fn view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "fake_workspace_id").await;
  let workspace_id = folder_test.get_workspace_id();

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let time = timestamp();
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);

  let views = folder_test.views.get_views_belong_to(&workspace_id);
  assert_eq!(views.len(), 2);

  assert_json_diff::assert_json_include!(
    actual: folder_test.to_json_value(),
    expected: json!({
          "meta": {
            "current_view": "",
            "current_workspace": "fake_workspace_id"
          },
          "relation": {
            "fake_workspace_id": [
              {
                "id": "v1"
              },
              {
                "id": "v2"
              }
            ],
            "v1": [],
            "v2": []
          },
          "section": {
            "favorite": {}
          },
          "trash": [],
          "views": {
            "fake_workspace_id": {
              "bid": "",
              "created_at": time,
              "desc": "",
              "icon": "",
              "id": "fake_workspace_id",
              "layout": 0,
              "name": ""
            },
            "v1": {
              "bid": "fake_workspace_id",
              "created_at": time,
              "desc": "",
              "icon": "",
              "id": "v1",
              "layout": 0,
              "name": ""
            },
            "v2": {
              "bid": "fake_workspace_id",
              "created_at": time,
              "desc": "",
              "icon": "",
              "id": "v2",
              "layout": 0,
              "name": ""
            }
          }
        })
  )
}

#[tokio::test]
async fn child_view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "fake_workspace_id").await;
  let workspace_id = folder_test.get_workspace_id();

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let view_2_1 = make_test_view("v2.1", "v2", vec![]);
  let view_2_2 = make_test_view("v2.2", "v2", vec![]);
  let time = timestamp();
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);
  folder_test.insert_view(view_2_1, None);
  folder_test.insert_view(view_2_2, None);

  // folder_test.workspaces.create_workspace(workspace);
  assert_json_diff::assert_json_include!(actual: folder_test.to_json_value(), expected: json!({
    "meta": {
      "current_view": "",
      "current_workspace": "fake_workspace_id"
    },
    "relation": {
      "fake_workspace_id": [
        {
          "id": "v1"
        },
        {
          "id": "v2"
        }
      ],
      "v1": [],
      "v2": [
        {
          "id": "v2.1"
        },
        {
          "id": "v2.2"
        }
      ],
      "v2.1": [],
      "v2.2": []
    },
    "section": {
      "favorite": {}
    },
    "trash": [],
    "views": {
      "fake_workspace_id": {
        "bid": "",
        "created_at": time,
        "desc": "",
        "icon": "",
        "id": "fake_workspace_id",
        "layout": 0,
        "name": ""
      },
      "v1": {
        "bid": "fake_workspace_id",
        "created_at": time,
        "desc": "",
        "icon": "",
        "id": "v1",
        "layout": 0,
        "name": ""
      },
      "v2": {
        "bid": "fake_workspace_id",
        "created_at": time,
        "desc": "",
        "icon": "",
        "id": "v2",
        "layout": 0,
        "name": ""
      },
      "v2.1": {
        "bid": "v2",
        "created_at": time,
        "desc": "",
        "icon": "",
        "id": "v2.1",
        "layout": 0,
        "name": ""
      },
      "v2.2": {
        "bid": "v2",
        "created_at": time,
        "desc": "",
        "icon": "",
        "id": "v2.2",
        "layout": 0,
        "name": ""
      }
    }
  }));
}
