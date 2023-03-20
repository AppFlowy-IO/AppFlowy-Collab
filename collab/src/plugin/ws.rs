use crate::plugin::CollabPlugin;
use anyhow::Result;
use bytes::Bytes;
use parking_lot::RwLock;

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
}

impl CollabPlugin for CollabWebSocketPlugin {
    fn did_receive_new_update(&self, update: Bytes) {
        self.updates.write().push(update);
    }
}
