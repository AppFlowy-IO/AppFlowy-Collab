use parking_lot::RwLock;
use std::sync::Arc;
use yrs::types::{Change, ToJson};
use yrs::updates::decoder::Decode;

use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::MapRefExtension;
use yrs::{Array, Doc, Map, Observable, ReadTxn, StateVector, Transact, Update};

#[test]
fn array_observer_test() {
  let doc1 = Doc::with_client_id(1);
  let block_1 = doc1.get_or_insert_array("blocks");
  let mut txn = doc1.transact_mut();
  block_1.insert(&mut txn, 0, "1");
  block_1.insert(&mut txn, 1, "2");
  let update_1 = txn.encode_update_v1();
  drop(txn);

  let doc2 = Doc::with_client_id(2);
  let mut block_2 = doc2.get_or_insert_array("blocks");
  let _subscription = block_2.observe(|txn, event| {
    for event in event.delta(txn) {
      match event {
        Change::Added(values) => {
          println!("add: {:?}", values)
        },
        Change::Removed(value) => {
          println!("remove: {}", value);
        },
        Change::Retain(value) => {
          println!("retain : {}", value);
        },
      }
    }
  });

  let mut txn = doc2.transact_mut();
  txn.apply_update(Update::decode_v1(&update_1).unwrap());
  drop(txn);

  let mut txn = doc1.transact_mut();
  block_1.remove(&mut txn, 1);
  let update_2 = txn.encode_update_v1();
  drop(txn);

  let mut txn = doc2.transact_mut();
  txn.apply_update(Update::decode_v1(&update_2).unwrap());
  drop(txn);

  //Output:
  // add: [Any(String("1")), Any(String("2"))]
  // retain : 1
  // remove: 1
}

#[test]
fn apply_update_test() {
  let doc1 = Doc::with_client_id(1);
  let updates = Arc::new(RwLock::new(vec![]));

  let cloned_updates = updates.clone();
  let sub = doc1
    .observe_update_v1(move |_txn, event| {
      cloned_updates.write().push(event.update.clone());
    })
    .unwrap();

  let array = doc1.get_or_insert_array("array");
  let doc1_state = doc1.transact().encode_diff_v1(&StateVector::default());
  {
    let mut txn = doc1.transact_mut();
    let map1 = array.insert_map_with_txn(&mut txn);
    // map1.insert(&mut txn, "m_k", "m_value");
    map1.insert_map_with_txn(&mut txn, "m_k");
  }

  {
    let mut txn = doc1.transact_mut();
    array.push_back(&mut txn, "a");
  }

  {
    let mut txn = doc1.transact_mut();
    array.push_back(&mut txn, "b");
  }

  assert_eq!(updates.read().len(), 3);
  assert_eq!(
    doc1.to_json(&doc1.transact()).to_string(),
    r#"{array: [{m_k: {}}, a, b]}"#
  );
  drop(sub);

  // *****************************************
  {
    let doc2 = Doc::new();
    let array = doc2.get_or_insert_array("array");
    {
      let mut txn = doc2.transact_mut();
      txn.apply_update(Update::decode_v1(doc1_state.as_ref()).unwrap());
      for update in updates.read().iter() {
        txn.apply_update(Update::decode_v1(update).unwrap());
      }
    }
    let map = {
      let txn = doc2.transact();
      let map = array
        .get(&txn, 0)
        .map(|value| value.to_ymap())
        .unwrap()
        .unwrap();

      assert_eq!(map.to_json(&txn).to_string(), r#"{m_k: {}}"#);
      map
    };

    let cloned_updates = updates.clone();
    let sub = doc2
      .observe_update_v1(move |_txn, event| {
        cloned_updates.write().push(event.update.clone());
      })
      .unwrap();
    let map_2 = {
      // update map
      let doc2 = doc2.clone();
      let mut txn = doc2.transact_mut();
      map.insert_map_with_txn(&mut txn, "m_m_k1")
    };

    {
      let mut txn = doc2.transact_mut();
      map_2.insert(&mut txn, "m_m_k2", "123");
    }
    {
      let mut txn = doc2.transact_mut();
      map_2.insert(&mut txn, "m_m_k2", "m_m_v2");
    }

    assert_eq!(updates.read().len(), 6);
    // assert_eq!(
    //   doc2.to_json(&doc2.transact()).to_string(),
    //   r#"{array: [{m_m_k1: {m_m_k2: m_m_v2}, m_k: {}}, a, b]}"#
    // );
    drop(sub);
  }

  // *****************************************
  {
    let doc3 = Doc::new();
    let array = doc3.get_or_insert_array("array");
    {
      let mut txn = doc3.transact_mut();
      txn.apply_update(Update::decode_v1(doc1_state.as_ref()).unwrap());
      for update in updates.read().iter() {
        txn.apply_update(Update::decode_v1(update).unwrap());
      }
    }

    let map = {
      let txn = doc3.transact();
      array
        .get(&txn, 0)
        .map(|value| value.to_ymap())
        .unwrap()
        .unwrap()
        .get(&txn, "m_m_k1")
        .unwrap()
        .to_ymap()
        .unwrap()
    };

    assert_eq!(
      map.to_json(&doc3.transact()).to_string(),
      r#"{m_m_k2: m_m_v2}"#
    );
  }
}
