use collab::core::collab::{make_yrs_doc, MutexCollab, TransactionMutExt};
use collab::core::origin::CollabOrigin;
use collab::core::transaction::DocTransactionExtension;
use collab::preclude::Collab;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use yrs::updates::decoder::Decode;
use yrs::{Map, ReadTxn, Transact, Update};

#[tokio::main]
async fn main() {
  // let x = vec![1, 2, 3].into_boxed_slice();
  // Box::leak(x);

  let doc = MutexCollab::new(CollabOrigin::Empty, "test", vec![]);
  doc.lock().initialize();
  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  {
    doc.lock().insert("key1", generate_random_string(1024));
  }

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  {
    doc.lock().insert("key2", generate_random_string(1024));
  }

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  let doc_2 = MutexCollab::new(CollabOrigin::Empty, "test", vec![]);
  doc_2.lock().initialize();
  let encoded = Update::decode_v1(&doc.encode_collab_v1().doc_state).unwrap();
  {
    let lock_garud = doc_2.lock();
    let mut txn = lock_garud.get_doc().transact_mut();
    txn.try_apply_update(encoded).unwrap();
    drop(txn);
  }

  //
  doc_2
    .lock()
    .insert("key3", generate_random_string(3 * 1024));
  {
    let lock_guard = doc_2.lock();
    let txn = lock_guard.transact();
    let diff = txn.encode_diff_v1(&doc.lock().transact().state_vector());
    let encoded = Update::decode_v1(&diff).unwrap();
    doc.lock().with_origin_transact_mut(|txn| {
      txn.try_apply_update(encoded);
    });
  }

  assert_eq!(doc.lock().get("key1"), doc_2.lock().get("key1"));
  assert_eq!(doc.lock().get("key2"), doc_2.lock().get("key2"));
  assert_eq!(doc.lock().get("key3"), doc_2.lock().get("key3"));

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

pub fn generate_random_string(len: usize) -> String {
  let rng = thread_rng();
  rng
    .sample_iter(&Alphanumeric)
    .take(len)
    .map(char::from)
    .collect()
}
