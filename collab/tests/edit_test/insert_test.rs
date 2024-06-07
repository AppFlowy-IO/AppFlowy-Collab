use collab::core::origin::CollabOrigin;
use collab::error::CollabError;
use collab::preclude::Collab;
use std::time::Duration;
use yrs::{Map, MapRef, Observable};

use crate::util::{setup_log, Person, Position};

#[tokio::test]
async fn insert_text() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  let _sub = collab.observe_data(|txn, event| {
    event.target().iter(txn).for_each(|(a, b)| {
      println!("{}: {}", a, b);
    });
  });

  collab.insert("text", "hello world");
  let value = collab.get("text").unwrap();
  let s = value.to_string(&collab.transact());
  assert_eq!(s, "hello world".to_string());
}

#[tokio::test]
async fn insert_json_attrs() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "develop".to_string(),
      level: 3,
    },
  };
  let mut tx = collab.transaction_mut().await;
  collab
    .insert_json_with_path(&mut tx, ["person"], object.clone())
    .unwrap();
  let person: Person = collab.get_json_with_path(&*tx, ["person"]).unwrap();
  assert_eq!(person, object);

  let pos: Position = collab
    .get_json_with_path(&*tx, ["person", "position"])
    .unwrap();
  assert_eq!(pos, object.position);
}

#[tokio::test]
async fn observer_attr_mut() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "developer".to_string(),
      level: 3,
    },
  };
  let mut tx = collab.transaction_mut().await;
  collab
    .insert_json_with_path(&mut tx, ["person"], object)
    .unwrap();

  let map: MapRef = collab
    .get_value_with_path(&*tx, ["person", "position"])
    .unwrap()
    .cast()
    .unwrap();
  let _sub = map.observe(|txn, event| {
    event.target().iter(txn).for_each(|(a, b)| {
      println!("{}: {}", a, b);
    });
  });

  map.insert(&mut tx, "title", "manager");
}

#[tokio::test]
async fn remove_value() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  let object = Person {
    name: "nathan".to_string(),
    position: Position {
      title: "developer".to_string(),
      level: 3,
    },
  };
  let mut tx = collab.transaction_mut().await;
  collab
    .insert_json_with_path(&mut tx, ["person"], object)
    .unwrap();
  let map: Option<MapRef> = collab
    .get_value_with_path(&*tx, ["person", "position"])
    .and_then(|v| v.cast().ok());
  assert!(map.is_some());

  collab
    .remove_with_path(&mut tx, ["person", "position"])
    .unwrap();

  let map: Option<MapRef> = collab
    .get_value_with_path(&*tx, ["person", "position"])
    .and_then(|v| v.cast().ok());
  assert!(map.is_none());
}

#[tokio::test]
async fn undo_single_insert_text() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  collab.enable_undo_redo().await;
  collab.insert("text", "hello world").await;

  assert_json_diff::assert_json_eq!(
    collab.to_json(),
    serde_json::json!({
      "text": "hello world"
    }),
  );

  // Undo the insert operation
  assert!(collab.can_undo());
  collab.undo().unwrap();

  // The text should be empty
  assert_json_diff::assert_json_eq!(collab.to_json(), serde_json::json!({}),);
}

#[tokio::test]
async fn redo_single_insert_text() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
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
#[should_panic]
async fn undo_manager_not_enable_test() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  collab.insert("text", "hello world").await;
  collab.undo().unwrap();
}

#[tokio::test]
async fn undo_second_insert_text() {
  let mut collab = Collab::new(1, "1", "1", vec![], false);
  collab.insert("1", "a").await;

  collab.enable_undo_redo().await;
  collab.insert("2", "b").await;
  collab.undo().unwrap();

  assert_json_diff::assert_json_eq!(
    collab.to_json(),
    serde_json::json!({
      "1": "a"
    }),
  );

  assert!(!collab.can_undo());
}
