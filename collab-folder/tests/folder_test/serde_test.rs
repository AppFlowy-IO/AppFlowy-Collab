use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, ReadTxn};
use collab_folder::{Folder, FolderData, UserId, ViewId, timestamp};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::util::{create_folder, make_test_view, parse_view_id};

#[test]
fn folder_json_serde() {
  let folder_test = create_folder(UserId::from(1), "fake_w_1");
  let time = timestamp();
  let fake_w_1_uuid =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "fake_w_1".as_bytes()).to_string();
  assert_json_diff::assert_json_include!(
    actual: folder_test.to_json_value(),
    expected: json!({
          "meta": {
            "current_workspace": &fake_w_1_uuid
          },
          "relation": {
            &fake_w_1_uuid: []
          },
          "section": {
            "favorite": {}
          },
          "views": {
            &fake_w_1_uuid: {
              "bid": "",
              "created_at": time,
              "icon": "",
              "id": &fake_w_1_uuid,
              "layout": 0,
              "name": ""
            }
          }
        }),
  );
}

#[test]
fn view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid.clone(), "fake_workspace_id");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let time = timestamp();
  {
    let mut txn = folder.collab.transact_mut();

    folder
      .body
      .views
      .insert(&mut txn, view_1, None, uid.as_i64());
    folder
      .body
      .views
      .insert(&mut txn, view_2, None, uid.as_i64());

    let views =
      folder
        .body
        .views
        .get_views_belong_to(&txn, &parse_view_id(&workspace_id), uid.as_i64());
    assert_eq!(views.len(), 2);
  }

  let fake_workspace_uuid =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "fake_workspace_id".as_bytes()).to_string();
  let v1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v1".as_bytes()).to_string();
  let v2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2".as_bytes()).to_string();
  assert_json_diff::assert_json_include!(
    actual: folder.to_json_value(),
    expected: json!({
          "meta": {
            "current_workspace": &fake_workspace_uuid
          },
          "relation": {
            &fake_workspace_uuid: [
              {
                "id": &v1_uuid
              },
              {
                "id": &v2_uuid
              }
            ],
            &v1_uuid: [],
            &v2_uuid: []
          },
          "section": {
            "favorite": {}
          },
          "views": {
            &fake_workspace_uuid: {
              "bid": "",
              "created_at": time,
              "icon": "",
              "id": &fake_workspace_uuid,
              "layout": 0,
              "name": ""
            },
            &v1_uuid: {
              "bid": &fake_workspace_uuid,
              "created_at": time,
              "icon": "",
              "id": &v1_uuid,
              "layout": 0,
              "name": ""
            },
            &v2_uuid: {
              "bid": &fake_workspace_uuid,
              "created_at": time,
              "icon": "",
              "id": &v2_uuid,
              "layout": 0,
              "name": ""
            }
          }
        })
  )
}

#[test]
fn child_view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid.clone(), "fake_workspace_id");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let mut folder = folder_test.folder;

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let view_2_1 = make_test_view("v2.1", "v2", vec![]);
  let view_2_2 = make_test_view("v2.2", "v2", vec![]);

  let time = timestamp();
  {
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
      .insert(&mut txn, view_2_1, None, uid.as_i64());
    folder
      .body
      .views
      .insert(&mut txn, view_2_2, None, uid.as_i64());
  }
  // folder_test.workspaces.create_workspace(workspace);
  let fake_workspace_uuid =
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "fake_workspace_id".as_bytes()).to_string();
  let v1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v1".as_bytes()).to_string();
  let v2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2".as_bytes()).to_string();
  let v2_1_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2.1".as_bytes()).to_string();
  let v2_2_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2.2".as_bytes()).to_string();
  assert_json_diff::assert_json_include!(actual: folder.to_json_value(), expected: json!({
    "meta": {
      "current_workspace": &fake_workspace_uuid
    },
    "relation": {
      &fake_workspace_uuid: [
        {
          "id": &v1_uuid
        },
        {
          "id": &v2_uuid
        }
      ],
      &v1_uuid: [],
      &v2_uuid: [
        {
          "id": &v2_1_uuid
        },
        {
          "id": &v2_2_uuid
        }
      ],
      &v2_1_uuid: [],
      &v2_2_uuid: []
    },
    "section": {
      "favorite": {}
    },
    "views": {
      &fake_workspace_uuid: {
        "bid": "",
        "created_at": time,
        "icon": "",
        "id": &fake_workspace_uuid,
        "layout": 0,
        "name": ""
      },
      &v1_uuid: {
        "bid": &fake_workspace_uuid,
        "created_at": time,
        "icon": "",
        "id": &v1_uuid,
        "layout": 0,
        "name": ""
      },
      &v2_uuid: {
        "bid": &fake_workspace_uuid,
        "created_at": time,
        "icon": "",
        "id": &v2_uuid,
        "layout": 0,
        "name": ""
      },
      &v2_1_uuid: {
        "bid": &v2_uuid,
        "created_at": time,
        "icon": "",
        "id": &v2_1_uuid,
        "layout": 0,
        "name": ""
      },
      &v2_2_uuid: {
        "bid": &v2_uuid,
        "created_at": time,
        "icon": "",
        "id": &v2_2_uuid,
        "layout": 0,
        "name": ""
      }
    }
  }));
}

#[tokio::test]
async fn deserialize_folder_data() {
  let json = include_str!("../folder_test/history_folder/folder_data.json");
  let folder_data: FolderData = serde_json::from_str(json).unwrap();
  let options = CollabOptions::new(Uuid::new_v4(), default_client_id());
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let uid = UserId::from(folder_data.uid);
  let folder = Arc::new(Folder::create(collab, None, folder_data));

  let mut handles = vec![];
  for _ in 0..40 {
    let folder = folder.clone();
    let clone_uid = uid.clone();
    let handle = tokio::spawn(async move {
      let start = Instant::now();
      let _trash_ids = folder
        .get_all_trash_sections(clone_uid.as_i64())
        .into_iter()
        .map(|trash| trash.id)
        .collect::<Vec<_>>();

      // get the private view ids
      let _private_view_ids = folder
        .get_all_private_sections(clone_uid.as_i64())
        .into_iter()
        .map(|view| view.id)
        .collect::<Vec<_>>();

      get_view_ids_should_be_filtered(&folder, clone_uid.as_i64());
      let elapsed = start.elapsed();
      Ok::<Duration, anyhow::Error>(elapsed)
    });
    handles.push(handle);
  }

  let results = futures::future::join_all(handles).await;
  for result in results {
    let elapsed = result.unwrap();
    println!("Time elapsed is: {:?}", elapsed);
  }
}

fn get_view_ids_should_be_filtered(folder: &Folder, uid: i64) -> Vec<ViewId> {
  let trash_ids = get_all_trash_ids(folder, uid);
  let other_private_view_ids = get_other_private_view_ids(folder, uid);
  [trash_ids, other_private_view_ids].concat()
}

fn get_other_private_view_ids(folder: &Folder, uid: i64) -> Vec<ViewId> {
  let my_private_view_ids = folder
    .get_my_private_sections(uid)
    .into_iter()
    .map(|view| view.id)
    .collect::<Vec<_>>();

  let all_private_view_ids = folder
    .get_all_private_sections(uid)
    .into_iter()
    .map(|view| view.id)
    .collect::<Vec<_>>();

  all_private_view_ids
    .into_iter()
    .filter(|id| !my_private_view_ids.contains(id))
    .collect()
}

fn get_all_trash_ids(folder: &Folder, uid: i64) -> Vec<ViewId> {
  let trash_ids = folder
    .get_all_trash_sections(uid)
    .into_iter()
    .map(|trash| trash.id)
    .collect::<Vec<_>>();
  let mut all_trash_ids = trash_ids.clone();
  let txn = folder.collab.transact();
  for trash_id in trash_ids {
    all_trash_ids.extend(get_all_child_view_ids(
      folder,
      &txn,
      &trash_id.to_string(),
      uid,
    ));
  }
  all_trash_ids
}

fn get_all_child_view_ids<T: ReadTxn>(
  folder: &Folder,
  txn: &T,
  view_id: &str,
  uid: i64,
) -> Vec<ViewId> {
  let child_views = folder
    .body
    .views
    .get_views_belong_to(txn, &parse_view_id(view_id), uid);
  let child_view_ids = child_views
    .iter()
    .map(|view| view.id)
    .collect::<Vec<ViewId>>();
  let mut all_child_view_ids = child_view_ids.clone();
  for child_view in child_views {
    all_child_view_ids.extend(get_all_child_view_ids(
      folder,
      txn,
      &child_view.id.to_string(),
      uid,
    ));
  }
  all_child_view_ids
}
