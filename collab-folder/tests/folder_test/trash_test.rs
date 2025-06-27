use std::future::Future;
use std::time::Duration;

use collab_folder::{SectionChange, SectionChangeReceiver, TrashSectionChange, UserId};

use crate::util::{create_folder_with_workspace, make_test_view};

#[test]
fn create_trash_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);

  let mut folder = folder_test.folder;

  folder.insert_view(view_1, Some(0), uid.as_i64());
  folder.insert_view(view_2, Some(0), uid.as_i64());
  folder.insert_view(view_3, Some(0), uid.as_i64());

  folder.add_trash_view_ids(
    vec!["v1".to_string(), "v2".to_string(), "v3".to_string()],
    uid.as_i64(),
  );

  let trash = folder.get_my_trash_sections(uid.as_i64());
  assert_eq!(trash.len(), 3);
  assert_eq!(trash[0].id, "v1");
  assert_eq!(trash[1].id, "v2");
  assert_eq!(trash[2].id, "v3");
}

#[test]
fn delete_trash_view_ids_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut folder = folder_test.folder;

  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  folder.insert_view(view_1, Some(0), uid.as_i64());
  folder.insert_view(view_2, Some(0), uid.as_i64());

  folder.add_trash_view_ids(vec!["v1".to_string(), "v2".to_string()], uid.as_i64());

  let trash = folder.get_my_trash_sections(uid.as_i64());
  assert_eq!(trash[0].id, "v1");
  assert_eq!(trash[1].id, "v2");

  folder.delete_trash_view_ids(vec!["v1".to_string()], uid.as_i64());
  let trash = folder.get_my_trash_sections(uid.as_i64());
  assert_eq!(trash[0].id, "v2");
}

#[tokio::test]
async fn create_trash_callback_test() {
  let uid = UserId::from(1);
  let mut folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let section_rx = folder_test.section_rx.take().unwrap();

  tokio::spawn(async move {
    folder_test.add_trash_view_ids(vec!["1".to_string(), "2".to_string()], uid.as_i64());
  });

  timeout(poll_tx(section_rx, |change| match change {
    SectionChange::Trash(change) => match change {
      TrashSectionChange::TrashItemAdded { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
      TrashSectionChange::TrashItemRemoved { .. } => {},
    },
  }))
  .await;
}

#[tokio::test]
async fn delete_trash_view_ids_callback_test() {
  let uid = UserId::from(1);
  let mut folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let trash_rx = folder_test.section_rx.take().unwrap();
  tokio::spawn(async move {
    folder_test.add_trash_view_ids(vec!["1".to_string(), "2".to_string()], uid.as_i64());
    folder_test.delete_trash_view_ids(vec!["1".to_string(), "2".to_string()], uid.as_i64());
  });

  timeout(poll_tx(trash_rx, |change| match change {
    SectionChange::Trash(change) => match change {
      TrashSectionChange::TrashItemAdded { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
      TrashSectionChange::TrashItemRemoved { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
    },
  }))
  .await;
}

async fn poll_tx(mut rx: SectionChangeReceiver, callback: impl Fn(SectionChange)) {
  while let Ok(change) = rx.recv().await {
    callback(change)
  }
}

async fn timeout<F: Future>(f: F) {
  tokio::time::timeout(Duration::from_secs(2), f)
    .await
    .unwrap();
}
