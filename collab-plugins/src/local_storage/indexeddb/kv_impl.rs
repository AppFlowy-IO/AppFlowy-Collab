use crate::local_storage::kv::PersistenceError;
use collab::entity::EncodedCollab;
use indexed_db_futures::prelude::*;
use js_sys::{ArrayBuffer, Uint8Array};

use crate::local_storage::kv::keys::{
  Clock, DOC_ID_LEN, DocID, clock_from_key, make_doc_end_key, make_doc_id_key_v1,
  make_doc_start_key, make_doc_state_key, make_doc_update_key, make_state_vector_key,
};
use crate::local_storage::kv::oid::{LOCAL_DOC_ID_GEN, OID};
use anyhow::anyhow;
use collab::core::collab::TransactionMutExt;
use collab::lock::RwLock;
use indexed_db_futures::web_sys::IdbKeyRange;
use std::sync::Arc;
use tracing::error;
use wasm_bindgen::{JsCast, JsValue};
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

pub struct CollabIndexeddb {
  db: Arc<RwLock<IdbDatabase>>,
}

unsafe impl Send for CollabIndexeddb {}
unsafe impl Sync for CollabIndexeddb {}

const COLLAB_KV_STORE: &str = "collab_kv";
impl CollabIndexeddb {
  pub async fn new() -> Result<Self, PersistenceError> {
    let mut db_req = IdbDatabase::open_u32("appflowy_indexeddb", 1)?;
    db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
      if evt
        .db()
        .object_store_names()
        .find(|n| n == COLLAB_KV_STORE)
        .is_none()
      {
        evt.db().create_object_store(COLLAB_KV_STORE)?;
      }
      Ok(())
    }));
    let db = Arc::new(RwLock::new(db_req.await?));
    Ok(Self { db })
  }

  pub async fn with_write_transaction<Output>(
    &self,
    f: impl FnOnce(&IdbTransactionActionImpl<'_>) -> Result<Output, PersistenceError>,
  ) -> Result<Output, PersistenceError> {
    let db_write_guard = self.db.write_err().await;
    let txn = db_write_guard
      .transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let action_impl = IdbTransactionActionImpl::new(txn)?;
    let output = f(&action_impl)?;
    action_impl.tx.await.into_result()?;
    Ok(output)
  }

  pub async fn get_data<K>(
    &self,
    store: &IdbObjectStore<'_>,
    key: K,
  ) -> Result<Vec<u8>, PersistenceError>
  where
    K: AsRef<[u8]>,
  {
    let js_key = to_js_value(key.as_ref());
    match store.get(&js_key)?.await? {
      None => Err(PersistenceError::RecordNotFound(format!(
        "object with given key:{:?} is not found",
        js_key
      ))),
      Some(value) => Ok(Uint8Array::new(&value).to_vec()),
    }
  }

  pub async fn set_data<K, V>(&self, key: K, value: V) -> Result<(), PersistenceError>
  where
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
  {
    let write_guard = self.db.write_err().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    self.set_data_with_store(&store, key, value).await?;
    transaction_result_to_result(transaction.await)?;
    Ok(())
  }

  pub async fn set_data_with_store<K, V>(
    &self,
    store: &IdbObjectStore<'_>,
    key: K,
    value: V,
  ) -> Result<(), PersistenceError>
  where
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
  {
    let js_key = to_js_value(key.as_ref());
    let js_value = to_js_value(value.as_ref());
    store.put_key_val(&js_key, &js_value)?.await?;
    Ok(())
  }

  pub async fn create_doc(
    &self,
    uid: i64,
    object_id: &str,
    encoded_collab: &EncodedCollab,
  ) -> Result<(), PersistenceError> {
    let doc_id = self.create_doc_id(uid, object_id).await?;
    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);

    let read_guard = self.db.write_err().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    self
      .set_data_with_store(&store, doc_state_key, &encoded_collab.doc_state)
      .await?;
    self
      .set_data_with_store(&store, sv_key, &encoded_collab.state_vector)
      .await?;

    transaction_result_to_result(transaction.await)?;
    Ok(())
  }

  pub async fn load_doc(
    &self,
    uid: i64,
    object_id: &str,
    doc: Doc,
  ) -> Result<(), PersistenceError> {
    let read_guard = self.db.read_err().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    let store = store_from_transaction(&transaction)?;
    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;

    let doc_state_key = make_doc_state_key(doc_id);
    let doc_state = self.get_data(&store, doc_state_key).await?;
    let updates = fetch_updates(&store, doc_id).await?;

    let mut txn = doc
      .try_transact_mut()
      .map_err(|err| PersistenceError::Internal(anyhow!("Transact mut fail. error: {:?}", err)))?;
    let doc_state_update = Update::decode_v1(doc_state.as_ref()).map_err(PersistenceError::Yrs)?;
    txn.try_apply_update(doc_state_update)?;

    for update in updates {
      if let Ok(update) = Update::decode_v1(update.as_ref()) {
        txn.try_apply_update(update)?;
      }
    }

    drop(txn);
    Ok(())
  }

  pub async fn get_encoded_collab(
    &self,
    uid: i64,
    object_id: &str,
  ) -> Result<EncodedCollab, PersistenceError> {
    let read_guard = self.db.read_err().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    let store = store_from_transaction(&transaction)?;

    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;

    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);

    let doc_stata = self.get_data(&store, doc_state_key).await?;
    let sv = self.get_data(&store, sv_key).await?;

    Ok(EncodedCollab::new_v1(sv, doc_stata))
  }

  pub async fn is_exist(&self, uid: i64, object_id: &str) -> Result<bool, PersistenceError> {
    let read_guard = self.db.read_err().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    let store = store_from_transaction(&transaction)?;
    Ok(self.get_doc_id(&store, uid, object_id).await.is_some())
  }

  pub async fn delete_doc(&self, uid: i64, object_id: &str) -> Result<(), PersistenceError> {
    let write_guard = self.db.write_err().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;

    self.delete_all_updates(&store, doc_id).await?;

    // delete the doc state and state vector
    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);
    store.delete(&to_js_value(doc_state_key.as_ref()))?;
    store.delete(&to_js_value(sv_key.as_ref()))?;
    transaction_result_to_result(transaction.await)?;
    Ok(())
  }

  pub async fn flush_doc(
    &self,
    uid: i64,
    object_id: &str,
    encoded: &EncodedCollab,
  ) -> Result<(), PersistenceError> {
    let write_guard = self.db.write_err().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;
    self.delete_all_updates(&store, doc_id).await?;

    // save the new doc state and state vector
    let doc_state_key = make_doc_state_key(doc_id);
    let sv_key = make_state_vector_key(doc_id);
    self
      .set_data_with_store(&store, doc_state_key, &encoded.doc_state)
      .await?;
    self
      .set_data_with_store(&store, sv_key, &encoded.state_vector)
      .await?;
    transaction_result_to_result(transaction.await)?;
    Ok(())
  }

  pub async fn push_update(
    &self,
    uid: i64,
    object_id: &str,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let write_guard = self.db.write_err().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;
    self.put_update(&store, doc_id, update).await?;
    transaction_result_to_result(transaction.await)?;
    Ok(())
  }

  async fn delete_all_updates(
    &self,
    store: &IdbObjectStore<'_>,
    doc_id: DocID,
  ) -> Result<(), PersistenceError> {
    let start = to_js_value(make_doc_start_key(doc_id));
    let end = to_js_value(make_doc_end_key(doc_id));
    let key_range = IdbKeyRange::bound(&start, &end).map_err(|err| {
      PersistenceError::Internal(anyhow!("Get last update key fail. error: {:?}", err))
    })?;

    let cursor_request = store
      .open_cursor_with_range(&key_range)?
      .await?
      .ok_or_else(|| {
        PersistenceError::Internal(anyhow!("Open cursor fail. error: {:?}", "cursor is none"))
      })?;

    // Delete the first key
    let _ = cursor_request.delete();
    while cursor_request.continue_cursor()?.await? {
      if let Err(err) = cursor_request.delete() {
        error!("failed to delete cursor: {:?}", err)
      }
    }

    Ok(())
  }

  pub async fn get_all_updates(
    &self,
    uid: i64,
    object_id: &str,
  ) -> Result<Vec<Vec<u8>>, PersistenceError> {
    let read_guard = self.db.read_err().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    let store = store_from_transaction(&transaction)?;
    let doc_id = self
      .get_doc_id(&store, uid, object_id)
      .await
      .ok_or_else(|| {
        PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
      })?;

    fetch_updates(&store, doc_id).await
  }

  async fn put_update(
    &self,
    store: &IdbObjectStore<'_>,
    id: OID,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let max_key = JsValue::from(Uint8Array::from(
      make_doc_update_key(id, Clock::MAX).as_ref(),
    ));

    let key_range = IdbKeyRange::upper_bound(&max_key).map_err(|err| {
      PersistenceError::Internal(anyhow!("Get last update key fail. error: {:?}", err))
    })?;
    let cursor = store
      .open_cursor_with_range_and_direction(&key_range, IdbCursorDirection::Prev)?
      .await?
      .ok_or_else(|| {
        PersistenceError::Internal(anyhow!("Open cursor fail. error: {:?}", "cursor is none"))
      })?;

    let clock = cursor
      .key()
      .map(|key| {
        let array_buffer = key.dyn_into::<ArrayBuffer>().unwrap();
        let uint8_array = Uint8Array::new(&array_buffer);
        let mut vec = vec![0; uint8_array.length() as usize];
        uint8_array.copy_to(&mut vec);
        let clock_byte = clock_from_key(&vec);
        Clock::from_be_bytes(clock_byte.try_into().unwrap())
      })
      .unwrap_or_else(|| 0);

    let next_clock = clock + 1;
    let update_key = make_doc_update_key(id, next_clock);
    self.set_data_with_store(store, update_key, update).await?;
    Ok(())
  }

  pub async fn get_doc_id<K>(
    &self,
    store: &IdbObjectStore<'_>,
    uid: i64,
    workspace_id: &K,
    object_id: &K,
  ) -> Option<DocID>
  where
    K: AsRef<[u8]> + ?Sized,
  {
    let uid_id_bytes = &uid.to_be_bytes();
    let key = make_doc_id_key_v1(uid_id_bytes, workspace_id.as_ref(), object_id.as_ref());
    let value = self.get_data(store, key).await.ok()?;
    let mut bytes = [0; DOC_ID_LEN];
    bytes[0..DOC_ID_LEN].copy_from_slice(value.as_ref());
    Some(OID::from_be_bytes(bytes))
  }
}

fn to_js_value<K: AsRef<[u8]>>(key: K) -> JsValue {
  JsValue::from(Uint8Array::from(key.as_ref()))
}

fn store_from_transaction<'a>(
  txn: &'a IdbTransaction<'a>,
) -> Result<IdbObjectStore<'a>, PersistenceError> {
  txn
    .object_store(COLLAB_KV_STORE)
    .map_err(PersistenceError::from)
}

pub struct IdbTransactionActionImpl<'a> {
  tx: IdbTransaction<'a>,
}

impl<'a> IdbTransactionActionImpl<'a> {
  fn new(tx: IdbTransaction<'a>) -> Result<Self, PersistenceError> {
    Ok(Self { tx })
  }
}

fn transaction_result_to_result(result: IdbTransactionResult) -> Result<(), PersistenceError> {
  match result {
    IdbTransactionResult::Success => Ok(()),
    IdbTransactionResult::Error(err) => Err(PersistenceError::from(err)),
    IdbTransactionResult::Abort => Err(PersistenceError::Internal(anyhow!("Transaction aborted"))),
  }
}

async fn fetch_updates(
  store: &IdbObjectStore<'_>,
  doc_id: DocID,
) -> Result<Vec<Vec<u8>>, PersistenceError> {
  let start = to_js_value(make_doc_update_key(doc_id, 0).as_ref());
  let end = to_js_value(make_doc_update_key(doc_id, Clock::MAX).as_ref());
  let key_range = IdbKeyRange::bound(&start, &end).map_err(|err| {
    PersistenceError::Internal(anyhow!("Get last update key fail. error: {:?}", err))
  })?;
  let cursor_request = store.open_cursor_with_range(&key_range)?.await?;
  if cursor_request.is_none() {
    return Ok(Vec::new());
  }

  let cursor_request = cursor_request.unwrap();
  let mut js_values = Vec::new();
  js_values.push(cursor_request.value());
  while cursor_request.continue_cursor()?.await? {
    js_values.push(cursor_request.value());
  }

  Ok(
    js_values
      .into_iter()
      .map(|js_value| js_value.dyn_into::<Uint8Array>().unwrap().to_vec())
      .collect(),
  )
}
