pub mod history;
pub mod persistence;

use bytes::Bytes;

pub trait CollabPlugin: Send + Sync + 'static {
    fn did_receive_new_update(&self, _update: Bytes) {}
}
