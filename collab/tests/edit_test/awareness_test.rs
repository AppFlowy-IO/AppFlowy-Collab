use collab::preclude::Collab;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn awareness_insert_test() {
  let mut collab = Collab::new(1, "1", "1", vec![]);
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab.observe_awareness(move |_awareness, event| {
    tx.send(event.clone()).unwrap();
  });

  let s = json!({"name": "nathan"}).to_string();
  collab.get_mut_awareness().set_local_state(s.clone());
  let state = collab.get_awareness().get_local_state().unwrap();
  assert_eq!(state, s);

  sleep(Duration::from_secs(1)).await;
  let event = rx.recv().unwrap();
  assert_eq!(event.updated().len(), 1);
}

#[tokio::test]
async fn awareness__test() {
  let mut collab = Collab::new(1, "1", "1", vec![]);
  let state = collab.get_awareness().get_local_state().unwrap();
  let state_json = serde_json::from_str::<Value>(state).unwrap();
  assert_eq!(state_json, json!({"uid": 1}));
}
