use collab_folder::{RepeatedViewIdentifier, UserId, ViewIdentifier, Workspace};
use serde_json::json;

use crate::util::{create_folder, make_test_view};

#[tokio::test]
async fn folder_json_serde() {
  let folder_test = create_folder(1.into(), "1").await;
  assert_json_diff::assert_json_eq!(
    json!({
      "relation": {},
      "meta": {},
      "trash": [],
      "views": {},
      "workspaces": [],
      "favorites_v2": {}
    }),
    folder_test.to_json_value()
  );
}

#[tokio::test]
async fn workspace_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let belongings = RepeatedViewIdentifier {
    items: vec![
      ViewIdentifier::new("v1".to_string()),
      ViewIdentifier::new("v2".to_string()),
    ],
  };
  let workspace = Workspace {
    id: "w1".to_string(),
    name: "My first workspace".to_string(),
    child_views: belongings,
    created_at: 123,
  };

  folder_test.workspaces.create_workspace(workspace);
  assert_json_diff::assert_json_eq!(
    json!( {
      "meta": {},
      "relation": {
        "w1": [
          {
            "id": "v1"
          },
          {
            "id": "v2"
          }
        ]
      },
      "trash": [],
      "views": {},
      "workspaces": [
        {
          "created_at": 123,
          "id": "w1",
          "name": "My first workspace"
        }
      ],
      "favorites_v2": {},
    }),
    folder_test.to_json_value()
  );
}

#[tokio::test]
async fn view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let belongings = RepeatedViewIdentifier {
    items: vec![
      ViewIdentifier::new("v1".to_string()),
      ViewIdentifier::new("v2".to_string()),
    ],
  };
  let workspace = Workspace {
    id: "w1".to_string(),
    name: "My first workspace".to_string(),
    child_views: belongings,
    created_at: 123,
  };

  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);

  folder_test.workspaces.create_workspace(workspace);
  assert_json_diff::assert_json_eq!(
    json!( {
      "meta": {},
      "relation": {
        "v1": [],
        "v2": [],
        "w1": [
          {
            "id": "v1"
          },
          {
            "id": "v2"
          }
        ]
      },
      "trash": [],
      "views": {
        "v1": {
          "bid": "w1",
          "created_at": 0,
          "desc": "",
          "id": "v1",
          "layout": 0,
          "name": "",
           "is_favorite": {
            "1": false
          },
          "icon": ""
        },
        "v2": {
          "bid": "w1",
          "created_at": 0,
          "desc": "",
          "id": "v2",
          "layout": 0,
          "name": "",
           "is_favorite": {
            "1": false
          },
          "icon": ""
        }
      },
      "workspaces": [
        {
          "created_at": 123,
          "id": "w1",
          "name": "My first workspace"
        }
      ],
      "favorites_v2": {},
    }),
    folder_test.to_json_value()
  );
}

#[tokio::test]
async fn child_view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let belongings = RepeatedViewIdentifier {
    items: vec![
      ViewIdentifier::new("v1".to_string()),
      ViewIdentifier::new("v2".to_string()),
    ],
  };
  let workspace = Workspace {
    id: "w1".to_string(),
    name: "My first workspace".to_string(),
    child_views: belongings,
    created_at: 123,
  };

  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_2_1 = make_test_view("v2.1", "v2", vec![]);
  let view_2_2 = make_test_view("v2.2", "v2", vec![]);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);
  folder_test.insert_view(view_2_1, None);
  folder_test.insert_view(view_2_2, None);

  folder_test.workspaces.create_workspace(workspace);
  assert_json_diff::assert_json_eq!(
    json!( {
      "meta": {},
      "relation": {
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
        "v2.2": [],
        "w1": [
          {
            "id": "v1"
          },
          {
            "id": "v2"
          }
        ]
      },
      "trash": [],
      "views": {
        "v1": {
          "bid": "w1",
          "created_at": 0,
          "desc": "",
          "id": "v1",
          "layout": 0,
          "name": "",
          "is_favorite": {
            "1": false
          },
          "icon": ""
        },
        "v2": {
          "bid": "w1",
          "created_at": 0,
          "desc": "",
          "id": "v2",
          "layout": 0,
          "name": "",
          "is_favorite": {
            "1": false
          },
          "icon": ""
        },
        "v2.1": {
          "bid": "v2",
          "created_at": 0,
          "desc": "",
          "id": "v2.1",
          "layout": 0,
          "name": "",
          "is_favorite": {
            "1": false
          },
          "icon": ""
        },
        "v2.2": {
          "bid": "v2",
          "created_at": 0,
          "desc": "",
          "id": "v2.2",
          "layout": 0,
          "name": "",
          "is_favorite": {
            "1": false
          },
          "icon": ""
        }
      },
      "workspaces": [
        {
          "created_at": 123,
          "id": "w1",
          "name": "My first workspace"
        }
      ],
      "favorites_v2": {}
    }),
    folder_test.to_json_value()
  );
}
