use serde_json::json;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Map, ReadTxn, Transact, Update};

#[tokio::test]
async fn state_vec_apply_test() {
  let doc1 = Doc::with_client_id(1);
  let doc2 = Doc::with_client_id(1);

  let map = { doc1.get_or_insert_map("map") };
  let _map_2 = { doc2.get_or_insert_map("map") };
  let sv_1 = doc1.transact().state_vector();
  assert_eq!(sv_1.len(), 0);
  {
    let mut txn = doc1.transact_mut();
    map.insert(&mut txn, "1", "a");
  }
  let sv_2 = doc1.transact().state_vector();
  assert_eq!(sv_2.len(), 1);

  {
    let mut txn = doc1.transact_mut();
    map.insert(&mut txn, "2", "b");
  }
  let sv_3 = doc1.transact().state_vector();
  assert_eq!(sv_3.len(), 1);

  let update = doc1
    .transact()
    .encode_state_as_update_v1(&doc2.transact().state_vector());
  {
    let mut txn = doc2.transact_mut();
    txn.apply_update(Update::decode_v1(&update).unwrap());
  }

  assert_json_diff::assert_json_eq!(
    doc2.to_json(&doc2.transact()),
    json!({
      "map": {
        "1": "a",
        "2": "b"
      }
    })
  );
}

// #[test]
// fn apply_update_order_test() {
//   let doc1 = Doc::with_client_id(1);
//   let doc2 = Doc::with_client_id(2);
//   let updates = Arc::new(RwLock::new(vec![]));
//   let cloned_updates = updates.clone();
//   let sub = doc1
//     .observe_update_v1(move |_txn, event| {
//       cloned_updates.write().push(event.update.clone());
//     })
//     .unwrap();
//
//   let cloned_updates = updates.clone();
//   let sub2 = doc2
//     .observe_update_v1(move |_txn, event| {
//       cloned_updates.write().push(event.update.clone());
//     })
//     .unwrap();
//
//   let map1 = { doc1.get_or_insert_map("map") };
//   let map2 = { doc2.get_or_insert_map("map") };
//   {
//     let mut txn = doc1.transact_mut();
//     map1.insert(&mut txn, "1", "a");
//   }
//
//   {
//     let mut txn = doc2.transact_mut();
//     map2.insert(&mut txn, "2", "b");
//   }
//
//   {
//     let mut txn = doc1.transact_mut();
//     map1.insert(&mut txn, "3", "c");
//   }
//
//   {
//     let mut txn = doc1.transact_mut();
//     map1.insert(&mut txn, "4", "d");
//   }
//
//   let o1 = updates.read()[0].clone();
//   let o2 = updates.read()[1].clone();
//   let o3 = updates.read()[2].clone();
//
//   let doc3 = Doc::with_client_id(3);
//   let map = { doc3.get_or_insert_map("map") };
//
//   let mut txn = doc3.transact_mut();
//   txn.apply_update(Update::decode_v1(&o1).unwrap());
//   txn.apply_update(Update::decode_v1(&o2).unwrap());
//   txn.apply_update(Update::decode_v1(&o3).unwrap());
//   drop(txn);
//
//   let json = doc3.to_json(&doc3.transact());
//   assert_json_diff::assert_json_eq!(json, json!(""));
// }
