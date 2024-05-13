use collab::preclude::Collab;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn awareness_insert_test() {
  let mut collab = Collab::new(1, "1", "1", vec![], true);
  collab.emit_awareness_state();
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab.observe_awareness(move |event| {
    tx.send(event.clone()).unwrap();
  });

  let s = json!({"name": "nathan"});
  collab.get_mut_awareness().set_local_state(s.to_string());
  let state = collab.get_awareness().local_state().unwrap();
  assert_eq!(state, s.to_string());

  sleep(Duration::from_secs(1)).await;
  let event = rx.recv().unwrap();
  assert_eq!(event.updated().len(), 1);
}

#[tokio::test]
async fn awareness_updates() {
  let mut c1 = Collab::new(1, "1", "1", vec![], true);
  c1.emit_awareness_state();
  let mut c2 = Collab::new(2, "1", "1", vec![], true);
  c2.emit_awareness_state();

  let sync = Arc::new(Mutex::new(None));
  let _u = {
    let ch = sync.clone();
    c1.observe_awareness(move |event| {
      let update = event.awareness_state().full_update().unwrap();
      let mut lock = ch.lock().unwrap();
      *lock = Some(update);
    })
  };

  let s1 = json!({"name": "nathan"}).to_string();
  c1.get_mut_awareness().set_local_state(s1.clone());
  let s2 = json!({"name": "bartosz"}).to_string();
  c2.get_mut_awareness().set_local_state(s2.clone());
  let u2 = c2.get_awareness().update().unwrap();
  c1.get_mut_awareness().apply_update(u2).unwrap();

  let lock = sync.lock().unwrap();
  let awareness_update = lock.as_ref().unwrap();
  assert_eq!(awareness_update.clients.len(), 2);
  let values = awareness_update
    .clients
    .values()
    .map(|e| e.json.clone())
    .collect::<HashSet<_>>();
  assert_eq!(values, HashSet::from([s1, s2]));
}

#[tokio::test]
async fn initial_awareness_test() {
  let mut collab = Collab::new(1, "1", "1", vec![], true);
  collab.emit_awareness_state();
  // by default, the awareness state contains the uid
  let state = collab.get_awareness().local_state().unwrap();
  assert_eq!(state, json!({"uid": 1}).to_string());
}

#[tokio::test]
async fn clean_awareness_state_test() {
  let mut collab = Collab::new(1, "1", "1", vec![], true);
  collab.emit_awareness_state();
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab.observe_awareness(move |event| {
    tx.send(event.clone()).unwrap();
  });
  collab.clean_awareness_state();
  let event = rx.recv().unwrap();
  assert_eq!(event.removed().len(), 1);

  assert!(collab.get_awareness().local_state().is_none());
}

#[tokio::test]
async fn clean_awareness_state_sync_test() {
  let mut doc_id_map_uid = HashMap::new();
  let mut collab_a = Collab::new(0, "1", "1", vec![], true);
  collab_a.emit_awareness_state();
  doc_id_map_uid.insert(collab_a.get_doc().client_id(), 0.to_string());
  let (tx, rx) = mpsc::sync_channel(1);
  let _update = collab_a.observe_awareness(move |event| {
    let update = event.awareness_update().unwrap();
    tx.send(update.clone()).unwrap();
  });
  collab_a.emit_awareness_state();

  // apply the awareness state from collab_a to collab_b
  let awareness_update = rx.recv().unwrap();
  let mut collab_b = Collab::new(1, "1", "2", vec![], true);
  collab_b.emit_awareness_state();
  doc_id_map_uid.insert(collab_b.get_doc().client_id(), 1.to_string());
  collab_b
    .get_mut_awareness()
    .apply_update(awareness_update.clone())
    .unwrap();

  // collab_a's awareness state should be synced to collab_b after applying the update
  let states = collab_b.get_awareness().clients();
  assert_eq!(states.len(), 2);
  for (id, json) in states {
    let uid = doc_id_map_uid.get(id).unwrap().parse::<i64>().unwrap();
    assert_eq!(json, &json!({"uid": uid}).to_string());
  }

  // collab_a clean the awareness state
  collab_a.clean_awareness_state();
  let awareness_update = rx.recv().unwrap();
  // apply the awareness state from collab_a to collab_b. collab_b's awareness state should be cleaned
  collab_b
    .get_mut_awareness()
    .apply_update(awareness_update.clone())
    .unwrap();

  let states = collab_b.get_awareness().clients();
  assert_eq!(states.len(), 1);
}
