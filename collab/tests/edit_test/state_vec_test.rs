use serde_json::json;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Map, MapPrelim, MapRef, ReadTxn, Transact, Update};

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
    let update = Update::decode_v1(&update).unwrap();
    let mut txn = doc2.transact_mut();
    txn.apply_update(update).unwrap();
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

// The result is undetermined because the two peers are in a different state. Check out the
// two_way_sync_test for a more detailed explanation.
#[tokio::test]
async fn two_way_sync_result_undetermined() {
  let doc_1 = Doc::new();
  let doc_2 = Doc::new();
  let root_map_1 = doc_1.get_or_insert_map("root");
  let root_map_2 = doc_2.get_or_insert_map("root");

  // root: { map:{ } }
  let _map_1 = {
    let mut txn = doc_1.transact_mut();
    root_map_1.insert(&mut txn, "map", MapPrelim::default())
  };

  // root: { map:{ } }
  let map_2 = {
    let mut txn = doc_2.transact_mut();
    root_map_2.insert(&mut txn, "map", MapPrelim::default())
  };

  {
    let mut txn = doc_2.transact_mut();
    map_2.insert(&mut txn, "key_1", "a");
    map_2.insert(&mut txn, "key_2", "b");
  }

  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update).unwrap();
  }

  // When synchronizing updates, what happens is that a conflict has occurred - under the same key
  // "map" two different maps where inserted - map_1 and map_2 are logically different entities (in
  // Yjs/Yrs only root types are logically equivalent by their name). In order to resolve this conflict,
  // an update that created a nested map from the client with higher ID will override the one that came
  // from client with lower ID. If that happens, the overridden map will be tombstoned together with
  // all its elements.
  //
  // That Doc::new() generates random client ID for the document. So the two way sync is reuqired
  {
    let sv_2 = doc_2.transact().state_vector();
    let sv_update = doc_1.transact().encode_state_as_update_v1(&sv_2);
    let mut txn = doc_2.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update).unwrap();
  }

  // The a and b must be the same and might be empty. This is the result of the two way sync.
  let a = {
    let txn = doc_1.transact();
    root_map_1.to_json(&txn)
  };

  let b = {
    let txn = doc_2.transact();
    root_map_2.to_json(&txn)
  };

  println!("a: {}", a);
  println!("b: {}", b);
  // case 1:
  // a: {map: {}}
  // b: {map: {}}

  // case 2:
  // a: {map: {key_2: b, key_1: a}}
  // b: {map: {key_1: a, key_2: b}}
  assert_eq!(a, b);
}

#[tokio::test]
async fn two_way_sync_test() {
  let doc_1 = Doc::new();
  let doc_2 = Doc::new();
  let root_map_1 = doc_1.get_or_insert_map("root");
  let root_map_2 = doc_2.get_or_insert_map("root");

  // root: { map:{ } }
  let _map_1 = {
    let mut txn = doc_1.transact_mut();
    root_map_1.insert(&mut txn, "map", MapPrelim::default())
  };

  // sync the doc_1 local state to doc_2. Then the "map" will be treated as the same object.
  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update).unwrap();
  }
  {
    let sv_2 = doc_2.transact().state_vector();
    let sv_update = doc_1.transact().encode_state_as_update_v1(&sv_2);
    let mut txn = doc_2.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update).unwrap();
  }

  // Update the "map" in doc_2 and then sync to doc_1
  let map_2: MapRef = root_map_2
    .get(&doc_2.transact(), "map")
    .unwrap()
    .cast()
    .unwrap();
  {
    let mut txn = doc_2.transact_mut();
    map_2.insert(&mut txn, "key_1", "a");
    map_2.insert(&mut txn, "key_2", "b");
  }
  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update).unwrap();
  }

  // The a and b must be the same and not empty
  let a = {
    let txn = doc_1.transact();
    root_map_1.to_json(&txn)
  };

  let b = {
    let txn = doc_2.transact();
    root_map_2.to_json(&txn)
  };

  println!("a: {}", a);
  println!("b: {}", b);

  // a: {map: {key_1: a, key_2: b}}
  // b: {map: {key_2: b, key_1: a}}
  assert_eq!(a, b);
}
