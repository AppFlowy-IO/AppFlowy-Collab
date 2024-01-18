use crate::local_storage::kv::{KVTransactionDB, PersistenceError};
use collab::core::collab_plugin::EncodedCollab;
use indexed_db_futures::js_sys::wasm_bindgen::JsValue;
use indexed_db_futures::prelude::*;
use js_sys::Uint8Array;

pub struct CollabIndexeddb {
  db: IdbDatabase,
}
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
    let db = db_req.await?;
    Ok(Self { db })
  }

  pub fn read_txn(&self) -> Result<IdbTransactionActionImpl<'_>, PersistenceError> {
    let tx = self
      .db
      .transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readonly)?;
    Ok(IdbTransactionActionImpl::new(tx)?)
  }

  pub async fn with_write_txn<Output>(
    &self,
    f: impl FnOnce(&IdbTransactionActionImpl<'_>) -> Result<Output, PersistenceError>,
  ) -> Result<Output, PersistenceError> {
    let tx = self
      .db
      .transaction_on_one_with_mode(COLLAB_KV_STORE, IdbTransactionMode::Readwrite)?;
    let action_impl = IdbTransactionActionImpl::new(tx)?;
    let output = f(&action_impl)?;
    action_impl.tx.await.into_result()?;
    Ok(output)
  }

  pub async fn get_encoded_collab(
    &self,
    object_id: &str,
  ) -> Result<EncodedCollab, PersistenceError> {
    let js_object_id = JsValue::from_str(object_id);
    let read_txn = self.read_txn()?;
    let store = read_txn.get_store()?;
    match store.get(&js_object_id)?.await? {
      None => Err(PersistenceError::RecordNotFound(format!(
        "object with given id: {} is not found",
        object_id
      ))),
      Some(value) => {
        let bytes = Uint8Array::new(&value).to_vec();
        let encoded = EncodedCollab::decode_from_bytes(&bytes)?;
        Ok(encoded)
      },
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
      .with_write_txn(|txn| {
        let store = txn.get_store()?;
        store.put_key_val(&object_id, (&buffer).into())?;
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
