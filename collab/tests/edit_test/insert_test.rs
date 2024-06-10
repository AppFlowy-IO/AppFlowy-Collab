use assert_matches2::assert_matches;
use collab::preclude::Collab;

use collab::core::collab::CollabReadOps;
use collab::error::CollabError;
use yrs::{Map, MapRef, Observable};

use crate::util::{Person, Position};

#[tokio::test]
async fn insert_text() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let _sub = collab.observe_data(|txn, event| {
    event.target().iter(txn).for_each(|(a, b)| {
      println!("{}: {}", a, b);
    });
  });

  let c = collab.write().await;
  c.insert_with_txn(&mut c.transact_mut(), "text", "hello world");
  let s: String = c.get_with_path(&c.transact(), ["text"]).unwrap();
  assert_eq!(s, "hello world".to_string());
}

#[tokio::test]
async fn insert_json_attrs() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "develop".to_string(),
      level: 3,
    },
  };
  let collab = collab.write().await;
  collab
    .insert_json_with_path(&mut collab.transact_mut(), ["person"], object.clone())
    .unwrap();

  let person: Person = collab
    .get_json_with_path(&collab.transact(), ["person"])
    .unwrap();
  assert_eq!(person, object);

  let pos: Position = collab
    .get_json_with_path(&collab.transact(), ["person", "position"])
    .unwrap();
  assert_eq!(pos, object.position);
}

#[tokio::test]
async fn observer_attr_mut() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "developer".to_string(),
      level: 3,
    },
  };
  let collab = collab.write().await;
  collab
    .insert_json_with_path(&mut collab.transact_mut(), ["person"], object)
    .unwrap();

  let map: MapRef = collab
    .get_with_path(&collab.transact(), ["person", "position"])
    .unwrap();
  let _sub = map.observe(|txn, event| {
    event.target().iter(txn).for_each(|(a, b)| {
      println!("{}: {}", a, b);
    });
  });

  map.insert(&mut collab.transact_mut(), "title", "manager");
}

#[tokio::test]
async fn remove_value() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "developer".to_string(),
      level: 3,
    },
  };
  let collab = collab.write().await;
  collab
    .insert_json_with_path(&mut collab.transact_mut(), ["person"], object)
    .unwrap();
  let map: Option<MapRef> = collab.get_with_path(&collab.transact(), ["person", "position"]);
  assert!(map.is_some());

  collab
    .remove_with_path(&mut collab.transact_mut(), ["person", "position"])
    .unwrap();

  let map: Option<MapRef> = collab.get_with_path(&collab.transact(), ["person", "position"]);
  assert!(map.is_none());
}

#[tokio::test]
async fn undo_single_insert_text() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let mut collab = collab.write().await;
  collab.enable_undo_redo();
  collab.insert("text", "hello world");

  assert_json_diff::assert_json_eq!(
    collab.to_json(),
    serde_json::json!({
      "text": "hello world"
    }),
  );

  // Undo the insert operation
  assert!(collab.undo().unwrap());

  // The text should be empty
  assert_json_diff::assert_json_eq!(collab.to_json(), serde_json::json!({}));
}

#[tokio::test]
async fn redo_single_insert_text() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let mut collab = collab.write().await;
  collab.enable_undo_redo();
  collab.insert("text", "hello world");

  // Undo the insert operation
  assert!(collab.can_undo());
  assert!(!collab.can_redo());

  collab.undo().unwrap();
  assert!(collab.can_redo());
  collab.redo().unwrap();

  assert_json_diff::assert_json_eq!(
    collab.to_json(),
    serde_json::json!({
      "text": "hello world"
    }),
  );
}

#[tokio::test]
async fn undo_manager_not_enable_test() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let mut collab = collab.write().await;
  collab.insert("text", "hello world");
  let result = collab.undo();
  assert_matches!(result, Err(CollabError::UndoManagerNotEnabled));
}

#[tokio::test]
async fn undo_second_insert_text() {
  let collab = Collab::new(1, "1", "1", vec![], false);
  let mut collab = collab.write().await;
  collab.insert("1", "a");

  collab.enable_undo_redo();
  collab.insert("2", "b");
  collab.undo().unwrap();

  assert_json_diff::assert_json_eq!(
    collab.to_json(),
    serde_json::json!({
      "1": "a"
    }),
  );

  assert!(!collab.can_undo());
}
