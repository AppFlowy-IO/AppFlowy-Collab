use serde_json::json;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Map, ReadTxn, Transact, Update};

#[test]
fn state_vec_aplay_test() {
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
  // assert_eq!(doc2.to_json(&doc2.transact()).to_string(), "{map: {1: a}}");
}
