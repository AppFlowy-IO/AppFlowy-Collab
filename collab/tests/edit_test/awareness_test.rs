use collab::core::awareness::gen_awareness_update_message;
use collab::preclude::Collab;
use serde_json::json;
use std::collections::HashMap;
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

  let s = json!({"name": "nathan"});
  collab.get_mut_awareness().set_local_state(s.clone());
  let state = collab.get_awareness().get_local_state().unwrap();
  assert_eq!(state, &s);

  sleep(Duration::from_secs(1)).await;
  let event = rx.recv().unwrap();
  assert_eq!(event.updated().len(), 1);
}

#[tokio::test]
async fn initial_awareness_test() {
  let collab = Collab::new(1, "1", "1", vec![]);

  // by default, the awareness state contains the uid
  let state = collab.get_awareness().get_local_state().unwrap();
  assert_eq!(state, &json!({"uid": 1}));
}

#[tokio::test]
async fn clean_awareness_state_test() {
  let mut collab = Collab::new(1, "1", "1", vec![]);
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab.observe_awareness(move |_awareness, event| {
    tx.send(event.clone()).unwrap();
  });
  collab.clean_awareness_state();
  let event = rx.recv().unwrap();
  assert_eq!(event.removed().len(), 1);

  assert!(collab.get_awareness().get_local_state().is_none());
}

#[tokio::test]
async fn clean_awareness_state_sync_test() {
  let mut doc_id_map_uid = HashMap::new();
  let mut collab_a = Collab::new(0, "1", "1", vec![]);
  doc_id_map_uid.insert(collab_a.get_doc().client_id(), 0.to_string());
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab_a.observe_awareness(move |awareness, event| {
    let update = gen_awareness_update_message(awareness, event).unwrap();
    tx.send(update).unwrap();
  });
  collab_a.emit_awareness_state();

  // apply the awareness state from collab_a to collab_b
  let awareness_update = rx.recv().unwrap();
  let mut collab_b = Collab::new(1, "1", "2", vec![]);
  doc_id_map_uid.insert(collab_b.get_doc().client_id(), 1.to_string());
  collab_b
    .get_mut_awareness()
    .apply_update(awareness_update)
    .unwrap();

  // collab_a's awareness state should be synced to collab_b after applying the update
  let states = collab_b.get_awareness().get_states();
  assert_eq!(states.len(), 2);
  for (id, json) in states {
    let uid = doc_id_map_uid.get(id).unwrap().parse::<i64>().unwrap();
    assert_eq!(json, &json!({"uid": uid}));
  }

  // collab_a clean the awareness state
  collab_a.clean_awareness_state();
  let awareness_update = rx.recv().unwrap();
  // apply the awareness state from collab_a to collab_b. collab_b's awareness state should be cleaned
  collab_b
    .get_mut_awareness()
    .apply_update(awareness_update)
    .unwrap();

  let states = collab_b.get_awareness().get_states();
  assert_eq!(states.len(), 1);
}
