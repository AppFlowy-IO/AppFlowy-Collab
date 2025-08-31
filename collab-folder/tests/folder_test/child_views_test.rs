use assert_json_diff::assert_json_include;
use collab_folder::{UserId, timestamp};
use serde_json::json;

use crate::util::{create_folder_with_workspace, make_test_view, parse_view_id};

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

  let workspace_uuid_str =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, workspace_id.as_bytes()).to_string();
  let folder_data = folder
    .body
    .get_folder_data(&txn, &workspace_uuid_str, uid.as_i64())
    .unwrap();
  let value = serde_json::to_value(folder_data).unwrap();
  let fake_w_1_uuid = workspace_uuid_str.clone();
  let id_1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string();
  let id_1_1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_1".as_bytes()).to_string();
  let id_1_2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_2".as_bytes()).to_string();
  let id_1_2_1_uuid =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_2_1".as_bytes()).to_string();
  let id_1_2_2_uuid =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_2_2".as_bytes()).to_string();
  let id_1_3_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_3".as_bytes()).to_string();
  assert_json_include!(
    actual: value,
    expected: json!({
      "current_view": null,
      "favorites": {},
      "views": [
        {
          "children": {
            "items": [
              {
                "id": &id_1_1_uuid
              },
              {
                "id": &id_1_2_uuid
              },
              {
                "id": &id_1_3_uuid
              }
            ]
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &fake_w_1_uuid
        },
        {
          "children": {
            "items": []
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_1_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &id_1_uuid
        },
        {
          "children": {
            "items": [
              {
                "id": &id_1_2_1_uuid
              },
              {
                "id": &id_1_2_2_uuid
              }
            ]
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_2_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &id_1_uuid
        },
        {
          "children": {
            "items": []
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_2_1_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &id_1_2_uuid
        },
        {
          "children": {
            "items": []
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_2_2_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &id_1_2_uuid
        },
        {
          "children": {
            "items": []
          },
          "created_at": time,
          "icon": null,
          "id": &id_1_3_uuid,
          "is_favorite": false,
          "layout": 0,
          "name": "",
          "parent_view_id": &id_1_uuid
        }
      ],
      "workspace": {
        "child_views": {
          "items": [
            {
              "id": &id_1_uuid
            }
          ]
        },
        "id": &fake_w_1_uuid,
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
  let v_1 = make_test_view("1", "w1", vec![]);

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
  assert_eq!(
    v_1_child_views[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_1".as_bytes()).to_string()
  );
  assert_eq!(
    v_1_child_views[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_2".as_bytes()).to_string()
  );
  assert_eq!(
    v_1_child_views[2].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_3".as_bytes()).to_string()
  );

  folder
    .body
    .views
    .move_child(&mut txn, &v_1.id, 2, 0);
  folder
    .body
    .views
    .move_child(&mut txn, &v_1.id, 0, 1);

  let v_1_child_views = folder
    .body
    .views
    .get_view(&txn, &v_1.id, uid.as_i64())
    .unwrap();
  assert_eq!(
    v_1_child_views.children[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_1".as_bytes()).to_string()
  );
  assert_eq!(
    v_1_child_views.children[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_3".as_bytes()).to_string()
  );
  assert_eq!(
    v_1_child_views.children[2].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_2".as_bytes()).to_string()
  );
}

#[test]
fn delete_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let workspace_id = folder_test.get_workspace_id().unwrap();
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

  folder.body.views.remove_child(&mut txn, &uuid::Uuid::parse_str(&workspace_id).unwrap(), 1);
  let w_1_child_views =
    folder
      .body
      .views
      .get_views_belong_to(&txn, &parse_view_id(&workspace_id), uid.as_i64());
  assert_eq!(
    w_1_child_views[0].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_1".as_bytes()).to_string()
  );
  assert_eq!(
    w_1_child_views[1].id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1_3".as_bytes()).to_string()
  );
}

#[test]
fn delete_child_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_1_id = view_1.id.to_string();
  let view_1_1 = make_test_view("v1_1", "v1", vec![]);
  let view_1_1_id = view_1_1.id;
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
    .get_views_belong_to(&txn, &parse_view_id(&view_1_id), uid.as_i64());
  assert_eq!(views.len(), 1);

  folder.body.views.delete_views(&mut txn, vec![view_1_1_id]);
  let views = folder
    .body
    .views
    .get_views_belong_to(&txn, &parse_view_id(&view_1_id), uid.as_i64());
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

  let workspace_uuid_str =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, workspace_id.as_bytes()).to_string();
  let child_views =
    folder
      .body
      .views
      .get_views_belong_to(&txn, &parse_view_id(&workspace_uuid_str), uid.as_i64());
  assert_eq!(child_views.len(), 1);

  let orphan_views = folder
    .body
    .views
    .get_orphan_views_with_txn(&txn, uid.as_i64());
  assert_eq!(orphan_views.len(), 1);

  // The folder data should contains the orphan view
  let folder_data = folder
    .body
    .get_folder_data(&txn, &workspace_uuid_str, uid.as_i64())
    .unwrap();
  let fake_w_1_uuid = workspace_uuid_str.clone();
  let id_1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "1".as_bytes()).to_string();
  let id_2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "2".as_bytes()).to_string();
  assert_json_include!(
    actual: json!(folder_data),
    expected: json!({
          "current_view": null,
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
              "id": &id_1_uuid,
              "is_favorite": false,
              "last_edited_by": 1,
              "layout": 0,
              "name": "",
              "parent_view_id": &fake_w_1_uuid
            },
            {
              "children": {
                "items": []
              },
              "created_by": 1,
              "icon": null,
              "id": &id_2_uuid,
              "is_favorite": false,
              "last_edited_by": 1,
              "layout": 0,
              "name": "",
              "parent_view_id": &id_2_uuid
            }
          ],
          "workspace": {
            "child_views": {
              "items": [
                {
                  "id": &id_1_uuid
                }
              ]
            },
            "created_by": 1,
            "id": &fake_w_1_uuid,
            "last_edited_by": 1,
            "name": ""
          }
        })
  );
}
