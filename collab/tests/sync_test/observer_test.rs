use yrs::types::Change;
use yrs::updates::decoder::Decode;

use yrs::{Array, Doc, Observable, Transact, Update};

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
