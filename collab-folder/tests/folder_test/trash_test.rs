use std::future::Future;
use std::time::Duration;

use collab_folder::core::{TrashChange, TrashChangeReceiver};

use crate::util::create_folder_with_workspace;

#[tokio::test]
async fn create_trash_test() {
  let folder_test = create_folder_with_workspace("1", "w1").await;
  folder_test.add_trash(vec!["1".to_string(), "2".to_string(), "3".to_string()]);

  let trash = folder_test.get_all_trash();
  assert_eq!(trash.len(), 3);
  assert_eq!(trash[0].id, "1");
  assert_eq!(trash[1].id, "2");
  assert_eq!(trash[2].id, "3");
}

#[tokio::test]
async fn delete_trash_test() {
  let folder_test = create_folder_with_workspace("1", "w1").await;
  folder_test.add_trash(vec!["1".to_string(), "2".to_string()]);

  let trash = folder_test.get_all_trash();
  assert_eq!(trash[0].id, "1");
  assert_eq!(trash[1].id, "2");

  folder_test.delete_trash(vec!["1".to_string()]);
  let trash = folder_test.get_all_trash();
  assert_eq!(trash[0].id, "2");
}

#[tokio::test]
async fn create_trash_callback_test() {
  let mut folder_test = create_folder_with_workspace("1", "w1").await;
  let trash_rx = folder_test.trash_rx.take().unwrap();
  tokio::spawn(async move {
    folder_test.add_trash(vec!["1".to_string(), "2".to_string()]);
  });

  timeout(poll_tx(trash_rx, |change| match change {
    TrashChange::DidCreateTrash { ids } => {
      assert_eq!(ids, vec!["1", "2"]);
    },
    TrashChange::DidDeleteTrash { .. } => {},
  }))
  .await;
}

#[tokio::test]
async fn delete_trash_callback_test() {
  let mut folder_test = create_folder_with_workspace("1", "w1").await;
  let trash_rx = folder_test.trash_rx.take().unwrap();
  tokio::spawn(async move {
    folder_test.add_trash(vec!["1".to_string(), "2".to_string()]);
    folder_test.delete_trash(vec!["1".to_string(), "2".to_string()]);
  });

  timeout(poll_tx(trash_rx, |change| match change {
    TrashChange::DidCreateTrash { ids } => {
      assert_eq!(ids, vec!["1", "2"]);
    },
    TrashChange::DidDeleteTrash { ids } => {
      assert_eq!(ids, vec!["1", "2"]);
    },
  }))
  .await;
}

async fn poll_tx(mut rx: TrashChangeReceiver, callback: impl Fn(TrashChange)) {
  while let Ok(change) = rx.recv().await {
    callback(change)
  }
}

async fn timeout<F: Future>(f: F) {
  tokio::time::timeout(Duration::from_secs(2), f)
    .await
    .unwrap();
}
