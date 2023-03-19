use crate::plugin::CollabPlugin;
use bytes::Bytes;
use parking_lot::RwLock;
use std::sync::Arc;
use yrs::updates::decoder::Decode;
use yrs::{merge_updates_v1, Update};

pub struct CollabDiskPlugin {}

#[derive(Debug, Default, Clone)]
pub struct CollabStateCachePlugin(Arc<RwLock<Vec<Bytes>>>);

impl CollabStateCachePlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_updates(&self) -> Result<Vec<Update>, anyhow::Error> {
        let mut updates = vec![];
        for encoded_data in self.0.read().iter() {
            updates.push(Update::decode_v1(encoded_data)?);
        }
        Ok(updates)
    }

    pub fn get_update(&self) -> Result<Update, anyhow::Error> {
        let read_guard = self.0.read();
        let updates = read_guard
            .iter()
            .map(|update| update.as_ref())
            .collect::<Vec<&[u8]>>();
        let encoded_data = merge_updates_v1(&updates)?;
        let update = Update::decode_v1(&encoded_data)?;
        Ok(update)
    }

    pub fn clear(&self) {
        self.0.write().clear()
    }
}

impl CollabPlugin for CollabStateCachePlugin {
    fn did_receive_new_update(&self, update: Bytes) {
        self.0.write().push(update);
    }
}
