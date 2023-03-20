use crate::plugin::CollabPlugin;
use anyhow::Result;
use bytes::Bytes;
use parking_lot::RwLock;

use yrs::Update;

pub trait WebSocketConnect {
    fn send(msg: Bytes) -> Result<()>;
}

pub struct CollabWebSocketPlugin {
    updates: RwLock<Vec<Bytes>>,
}

impl CollabWebSocketPlugin {
    pub fn new() -> Self {
        Self {
            updates: RwLock::new(vec![]),
        }
    }

    fn get_updates(&self) -> Result<Vec<Update>, anyhow::Error> {
        // we can use [merge_updates_v1] to merge these updates
        Ok(vec![])
    }
}

impl CollabPlugin for CollabWebSocketPlugin {
    fn did_receive_new_update(&self, update: Bytes) {
        self.updates.write().push(update);
    }
}
