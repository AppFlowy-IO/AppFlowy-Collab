use crate::local_storage::kv::{KVStore, PersistenceError};
use collab::core::collab_plugin::EncodedCollab;
use indexed_db_futures::js_sys::wasm_bindgen::JsValue;
use indexed_db_futures::prelude::*;

use js_sys::Uint8Array;

use crate::local_storage::kv::keys::{make_doc_id_key, DocID, DOC_ID_LEN};
use crate::local_storage::kv::oid::OID;
use indexed_db_futures::web_sys::IdbKeyRange;
use std::sync::Arc;
use tokio::sync::RwLock;

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

  pub async fn get_encoded_collab(
    &self,
    object_id: &str,
  ) -> Result<EncodedCollab, PersistenceError> {
    let bytes = self.get_data(object_id).await?;
    let encoded = EncodedCollab::decode_from_bytes(&bytes)?;
    Ok(encoded)
  }

  pub async fn get_data<K: AsRef<[u8]>>(&self, key: K) -> Result<Vec<u8>, PersistenceError> {
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
        js_key.as_string()
      ))),
      Some(value) => Ok(Uint8Array::new(&value).to_vec()),
    }
  }

  pub async fn save_encoded_collab(
    &self,
    object_id: &str,
    encoded_collab: &EncodedCollab,
  ) -> Result<(), PersistenceError> {
    let object_id = JsValue::from_str(object_id);
    let bytes = encoded_collab.encode_to_bytes()?;
    let buffer = Uint8Array::from(&bytes[..]).buffer();
    self
      .with_write_transaction(|txn| {
        let store = txn.get_store()?;
        store.put_key_val(&object_id, &buffer)?;
        Ok(())
      })
      .await
  }
}

pub struct IdbTransactionActionImpl<'a> {
  tx: IdbTransaction<'a>,
}

impl<'a> IdbTransactionActionImpl<'a> {
  fn new(tx: IdbTransaction<'a>) -> Result<Self, PersistenceError> {
    Ok(Self { tx })
  }

  fn get_store(&self) -> Result<IdbObjectStore<'_>, PersistenceError> {
    let store = self.tx.object_store(COLLAB_KV_STORE)?;
    Ok(store)
  }
}

async fn get_doc_id<K, S>(
  uid: i64,
  object_id: &K,
  collab_db: &Arc<CollabIndexeddb>,
) -> Option<DocID>
where
  K: AsRef<[u8]> + ?Sized,
{
  let uid_id_bytes = &uid.to_be_bytes();
  let key = make_doc_id_key(uid_id_bytes, object_id.as_ref());
  let value = collab_db.get_data(key).await.ok()?;
  let mut bytes = [0; DOC_ID_LEN];
  bytes[0..DOC_ID_LEN].copy_from_slice(value.as_ref());
  Some(OID::from_be_bytes(bytes))
}
