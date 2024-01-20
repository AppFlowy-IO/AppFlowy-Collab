use crate::local_storage::kv::{KVStore, PersistenceError};
use collab::core::collab_plugin::EncodedCollab;
use indexed_db_futures::js_sys::wasm_bindgen::JsValue;
use indexed_db_futures::prelude::*;
use js_sys::{ArrayBuffer, Uint8Array};

use crate::local_storage::kv::keys::{
  clock_from_key, make_doc_id_key, make_doc_state_key, make_doc_update_key, Clock, DocID, Key,
  DOC_ID_LEN,
};
use crate::local_storage::kv::oid::{LOCAL_DOC_ID_GEN, OID};
use anyhow::anyhow;
use indexed_db_futures::web_sys::IdbKeyRange;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasm_bindgen::JsCast;

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
      if let None = evt.db().object_store_names().find(|n| n == COLLAB_KV_STORE) {
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
    let db_write_guard = self.db.write().await;
    let txn = db_write_guard
      .transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let action_impl = IdbTransactionActionImpl::new(txn)?;
    let output = f(&action_impl)?;
    action_impl.tx.await.into_result()?;
    Ok(output)
  }

  pub async fn get_data<K>(&self, key: K) -> Result<Vec<u8>, PersistenceError>
  where
    K: AsRef<[u8]>,
  {
    let js_key = JsValue::from(Uint8Array::from(key.as_ref()));
    match self
      .db
      .read()
      .await
      .transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?
      .object_store(COLLAB_KV_STORE)?
      .get(&js_key)?
      .await?
    {
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
    let write_guard = self.db.write().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
    self.set_data_with_store(store, key, value).await?;
    Ok(())
  }

  pub async fn set_data_with_store<K, V>(
    &self,
    store: IdbObjectStore<'_>,
    key: K,
    value: V,
  ) -> Result<(), PersistenceError>
  where
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
  {
    let js_key = JsValue::from(Uint8Array::from(key.as_ref()));
    let js_value = JsValue::from(Uint8Array::from(value.as_ref()));
    store.put_key_val(&js_key, &js_value)?.await?;
    Ok(())
  }

  pub async fn flush_doc<K>(&self, uid: i64, object_id: &K)
  where
    K: AsRef<[u8]> + ?Sized,
  {
  }

  pub async fn push_update(
    &self,
    uid: i64,
    object_id: &str,
    update: &[u8],
  ) -> Result<(), PersistenceError> {
    let doc_id = self.get_doc_id(uid, object_id).await.ok_or_else(|| {
      PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
    })?;
    self.put_update(doc_id, update).await?;
    Ok(())
  }

  pub async fn get_all_updates(
    &self,
    uid: i64,
    object_id: &str,
  ) -> Result<Vec<Vec<u8>>, PersistenceError> {
    let doc_id = self.get_doc_id(uid, object_id).await.ok_or_else(|| {
      PersistenceError::RecordNotFound(format!("doc_id for object_id:{} is not found", object_id))
    })?;
    let start = JsValue::from(Uint8Array::from(make_doc_update_key(doc_id, 0).as_ref()));
    let end = JsValue::from(Uint8Array::from(
      make_doc_update_key(doc_id, Clock::MAX).as_ref(),
    ));
    let key_range = IdbKeyRange::bound(&start, &end).map_err(|err| {
      PersistenceError::Internal(anyhow!("Get last update key fail. error: {:?}", err))
    })?;
    let read_guard = self.db.read().await;
    let transaction =
      read_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    let store = store_from_transaction(&transaction)?;
    let cursor_request = store
      .open_cursor_with_range(&key_range)?
      .await?
      .ok_or_else(|| {
        PersistenceError::Internal(anyhow!("Open cursor fail. error: {:?}", "cursor is none"))
      })?;

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

  async fn put_update(&self, id: OID, update: &[u8]) -> Result<(), PersistenceError> {
    let max_key = JsValue::from(Uint8Array::from(
      make_doc_update_key(id, Clock::MAX).as_ref(),
    ));

    let key_range = IdbKeyRange::upper_bound(&max_key).map_err(|err| {
      PersistenceError::Internal(anyhow!("Get last update key fail. error: {:?}", err))
    })?;
    let write_guard = self.db.write().await;
    let transaction =
      write_guard.transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let store = store_from_transaction(&transaction)?;
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

  pub async fn get_doc_id<K>(&self, uid: i64, object_id: &K) -> Option<DocID>
  where
    K: AsRef<[u8]> + ?Sized,
  {
    let uid_id_bytes = &uid.to_be_bytes();
    let key = make_doc_id_key(uid_id_bytes, object_id.as_ref());
    let value = self.get_data(key).await.ok()?;
    let mut bytes = [0; DOC_ID_LEN];
    bytes[0..DOC_ID_LEN].copy_from_slice(value.as_ref());
    Some(OID::from_be_bytes(bytes))
  }

  pub async fn create_doc_id<I>(&self, uid: i64, object_id: I) -> Result<DocID, PersistenceError>
  where
    I: AsRef<[u8]>,
  {
    let new_id = LOCAL_DOC_ID_GEN.lock().next_id();
    let key = make_doc_id_key(&uid.to_be_bytes(), object_id.as_ref());
    self.set_data(key, new_id.to_be_bytes()).await?;
    Ok(new_id)
  }

  pub async fn get_encoded_collab(
    &self,
    object_id: &str,
  ) -> Result<EncodedCollab, PersistenceError> {
    let bytes = self.get_data(object_id).await?;
    let encoded = EncodedCollab::decode_from_bytes(&bytes)?;
    Ok(encoded)
  }

  pub async fn save_encoded_collab(
    &self,
    object_id: &str,
    encoded_collab: &EncodedCollab,
  ) -> Result<(), PersistenceError> {
    let bytes = encoded_collab.encode_to_bytes()?;
    self.set_data(object_id, bytes).await
  }
}

fn store_from_transaction<'a>(
  txn: &'a IdbTransaction<'a>,
) -> Result<IdbObjectStore<'a>, PersistenceError> {
  txn
    .object_store(COLLAB_KV_STORE)
    .map_err(|err| PersistenceError::from(err))
}

pub struct IdbTransactionActionImpl<'a> {
  tx: IdbTransaction<'a>,
}

impl<'a> IdbTransactionActionImpl<'a> {
  fn new(tx: IdbTransaction<'a>) -> Result<Self, PersistenceError> {
    Ok(Self { tx })
  }
}
