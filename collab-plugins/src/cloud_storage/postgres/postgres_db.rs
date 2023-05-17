use std::sync::Arc;

use anyhow::{Error, Result};
use async_trait::async_trait;
use collab_sync::client::sink::MsgId;
use postgrest::Postgrest;

use crate::cloud_storage::remote_collab::RemoteCollabStorage;

pub struct PostgresDB {
    #[allow(dead_code)]
    object_id: String,
    #[allow(dead_code)]
    postgrest: Arc<Postgrest>,
}

impl PostgresDB {
    #[allow(dead_code)]
    pub fn new(object_id: String, postgrest: Arc<Postgrest>) -> Result<Self> {
        Ok(Self {
            object_id,
            postgrest,
        })
    }

}

#[async_trait]
impl RemoteCollabStorage for PostgresDB {
    async fn get_all_updates(&self, _object_id: &str) -> Result<Vec<Vec<u8>>, Error> {
        todo!()
    }

    async fn send_update(&self, _id: MsgId, _update: Vec<u8>) -> Result<(), Error> {
        todo!()
    }

    async fn flush(&self, _object_id: &str) {
        todo!()
    }
}