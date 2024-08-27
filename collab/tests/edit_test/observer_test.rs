use std::sync::{Arc, Mutex};

use yrs::types::{Change, ToJson};
use yrs::updates::decoder::Decode;
use yrs::{Array, Doc, Map, MapPrelim, MapRef, Observable, ReadTxn, StateVector, Transact, Update};

#[tokio::test]
async fn array_observer_test() {
  let doc1 = Doc::with_client_id(1);
  let block_1 = doc1.get_or_insert_array("blocks");
  let mut txn = doc1.transact_mut();
  block_1.insert(&mut txn, 0, "1");
  block_1.insert(&mut txn, 1, "2");
  let update_1 = txn.encode_update_v1();
  drop(txn);

  let doc2 = Doc::with_client_id(2);
  let block_2 = doc2.get_or_insert_array("blocks");
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
  let update = Update::decode_v1(&update_1).unwrap();
  txn.apply_update(update).unwrap();
  drop(txn);

  let mut txn = doc1.transact_mut();
  block_1.remove(&mut txn, 1);
  let update_2 = txn.encode_update_v1();
  drop(txn);

  let update = Update::decode_v1(&update_2).unwrap();
  let mut txn = doc2.transact_mut();
  txn.apply_update(update).unwrap();
  drop(txn);

  //Output:
  // add: [Any(String("1")), Any(String("2"))]
  // retain : 1
  // remove: 1
}

#[tokio::test]
async fn apply_update_test() {
  let doc1 = Doc::new();
  let updates = Arc::new(Mutex::new(vec![]));

  let cloned_updates = updates.clone();
  let sub = doc1
    .observe_update_v1(move |_txn, event| {
      cloned_updates.lock().unwrap().push(event.update.clone());
    })
    .unwrap();

  let array = doc1.get_or_insert_array("array");
  let doc1_state = doc1.transact().encode_diff_v1(&StateVector::default());
  {
    let mut txn = doc1.transact_mut();
    let map1 = array.push_back(&mut txn, MapPrelim::default());
    // map1.insert(&mut txn, "m_k", "m_value");
    map1.insert(&mut txn, "m_k", MapPrelim::default());
  }

  {
    let mut txn = doc1.transact_mut();
    array.push_back(&mut txn, "a");
  }

  {
    let mut txn = doc1.transact_mut();
    array.push_back(&mut txn, "b");
  }

  assert_eq!(updates.lock().unwrap().len(), 3);
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
      let update = Update::decode_v1(doc1_state.as_ref()).unwrap();
      let mut txn = doc2.transact_mut();
      txn.apply_update(update).unwrap();
      let lock = updates.lock().unwrap();
      for update in lock.iter() {
        let update = Update::decode_v1(update).unwrap();
        txn.apply_update(update).unwrap();
      }
    }
    let map = {
      let txn = doc2.transact();
      let map = array
        .get(&txn, 0)
        .map(|value| value.cast::<MapRef>())
        .unwrap()
        .unwrap();

      assert_eq!(map.to_json(&txn).to_string(), r#"{m_k: {}}"#);
      map
    };

    let cloned_updates = updates.clone();
    let sub = doc2
      .observe_update_v1(move |_txn, event| {
        cloned_updates.lock().unwrap().push(event.update.clone());
      })
      .unwrap();
    let map_2 = {
      // update map
      let doc2 = doc2.clone();
      let mut txn = doc2.transact_mut();
      map.insert(&mut txn, "m_m_k1", MapPrelim::default())
    };

    {
      let mut txn = doc2.transact_mut();
      map_2.insert(&mut txn, "m_m_k2", "123");
    }
    {
      let mut txn = doc2.transact_mut();
      map_2.insert(&mut txn, "m_m_k2", "m_m_v2");
    }

    assert_eq!(updates.lock().unwrap().len(), 6);
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
      let update = Update::decode_v1(doc1_state.as_ref()).unwrap();
      let mut txn = doc3.transact_mut();
      txn.apply_update(update).unwrap();
      let lock = updates.lock().unwrap();
      for update in lock.iter() {
        let update = Update::decode_v1(update).unwrap();
        txn.apply_update(update).unwrap();
      }
    }

    let map = {
      let txn = doc3.transact();
      array
        .get(&txn, 0)
        .map(|value| value.cast::<MapRef>())
        .unwrap()
        .unwrap()
        .get(&txn, "m_m_k1")
        .unwrap()
        .cast::<MapRef>()
        .unwrap()
    };

    assert_eq!(
      map.to_json(&doc3.transact()).to_string(),
      r#"{m_m_k2: m_m_v2}"#
    );
  }
}
