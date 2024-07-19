use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, ReadTxn};
use collab_folder::{timestamp, Folder, FolderData, UserId};
use serde_json::json;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::util::{create_folder, make_test_view};

#[test]
fn folder_json_serde() {
  let folder_test = create_folder(1.into(), "fake_w_1");
  let time = timestamp();
  assert_json_diff::assert_json_include!(
    actual: folder_test.to_json_value(),
    expected: json!({
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

#[test]
fn view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "fake_workspace_id");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let time = timestamp();
  {
    let mut lock = folder_test.inner.blocking_lock();
    let mut txn = lock.transact_mut();

    folder_test.views.insert(&mut txn, view_1, None);
    folder_test.views.insert(&mut txn, view_2, None);

    let views = folder_test.views.get_views_belong_to(&txn, &workspace_id);
    assert_eq!(views.len(), 2);
  }

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

#[test]
fn child_view_json_serde() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "fake_workspace_id");
  let workspace_id = folder_test.get_workspace_id().unwrap();

  let view_1 = make_test_view("v1", &workspace_id, vec![]);
  let view_2 = make_test_view("v2", &workspace_id, vec![]);
  let view_2_1 = make_test_view("v2.1", "v2", vec![]);
  let view_2_2 = make_test_view("v2.2", "v2", vec![]);

  let time = timestamp();
  {
    let mut lock = folder_test.inner.blocking_lock();
    let mut txn = lock.transact_mut();

    folder_test.views.insert(&mut txn, view_1, None);
    folder_test.views.insert(&mut txn, view_2, None);
    folder_test.views.insert(&mut txn, view_2_1, None);
    folder_test.views.insert(&mut txn, view_2_2, None);
  }
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

#[tokio::test]
async fn deserialize_folder_data() {
  let json = include_str!("../folder_test/history_folder/folder_data.json");
  let folder_data: FolderData = serde_json::from_str(json).unwrap();
  let collab = Arc::new(Mutex::new(Collab::new_with_origin(
    CollabOrigin::Empty,
    "1",
    vec![],
    true,
  )));
  let folder = Arc::new(Folder::create(1, collab.clone(), None, folder_data));

  let mut handles = vec![];
  for _ in 0..40 {
    let folder = folder.clone();
    let handle = tokio::spawn(async move {
      let inner = folder.inner.blocking_lock(); //TODO: do I need to say that this need to double lock is bad?
      let txn = inner.transact();
      let start = Instant::now();
      let _trash_ids = folder
        .get_all_trash_sections(&inner.transact())
        .into_iter()
        .map(|trash| trash.id)
        .collect::<Vec<String>>();

      // get the private view ids
      let _private_view_ids = folder
        .get_all_private_sections(&txn)
        .into_iter()
        .map(|view| view.id)
        .collect::<Vec<String>>();

      get_view_ids_should_be_filtered(&folder, &txn);
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

fn get_view_ids_should_be_filtered<T: ReadTxn>(folder: &Folder, txn: &T) -> Vec<String> {
  let trash_ids = get_all_trash_ids(folder, txn);
  let other_private_view_ids = get_other_private_view_ids(folder, txn);
  [trash_ids, other_private_view_ids].concat()
}

fn get_other_private_view_ids<T: ReadTxn>(folder: &Folder, txn: &T) -> Vec<String> {
  let my_private_view_ids = folder
    .get_my_private_sections(txn)
    .into_iter()
    .map(|view| view.id)
    .collect::<Vec<String>>();

  let all_private_view_ids = folder
    .get_all_private_sections(txn)
    .into_iter()
    .map(|view| view.id)
    .collect::<Vec<String>>();

  all_private_view_ids
    .into_iter()
    .filter(|id| !my_private_view_ids.contains(id))
    .collect()
}

fn get_all_trash_ids<T: ReadTxn>(folder: &Folder, txn: &T) -> Vec<String> {
  let trash_ids = folder
    .get_all_trash_sections(txn)
    .into_iter()
    .map(|trash| trash.id)
    .collect::<Vec<String>>();
  let mut all_trash_ids = trash_ids.clone();
  for trash_id in trash_ids {
    all_trash_ids.extend(get_all_child_view_ids(folder, txn, &trash_id));
  }
  all_trash_ids
}

fn get_all_child_view_ids<T: ReadTxn>(folder: &Folder, txn: &T, view_id: &str) -> Vec<String> {
  let child_view_ids = folder
    .views
    .get_views_belong_to(txn, view_id)
    .into_iter()
    .map(|view| view.id.clone())
    .collect::<Vec<String>>();
  let mut all_child_view_ids = child_view_ids.clone();
  for child_view_id in child_view_ids {
    all_child_view_ids.extend(get_all_child_view_ids(folder, txn, &child_view_id));
  }
  all_child_view_ids
}

#[derive(Clone)]
#[allow(clippy::arc_with_non_send_sync)]
struct MutexFolder(Arc<Mutex<Folder>>);

impl MutexFolder {
  #[allow(clippy::arc_with_non_send_sync)]
  fn new(folder: Folder) -> Self {
    Self(Arc::new(Mutex::new(folder)))
  }
}
impl Deref for MutexFolder {
  type Target = Arc<Mutex<Folder>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
unsafe impl Sync for MutexFolder {}
unsafe impl Send for MutexFolder {}
