mod error;
mod util;

pub mod core;
pub mod plugin_impl;

pub mod preclude {
    pub use crate::core::collab::{Collab, CollabBuilder, CollabContext};
    pub use crate::core::collab_plugin::CollabPlugin;
    pub use crate::core::map_wrapper::CustomMapRef;
    pub use crate::core::map_wrapper::MapRefWrapper;
    pub use crate::util::insert_json_value_to_map_ref;
    pub use yrs::{merge_updates_v1, Map, ReadTxn, StateVector, TransactionMut, Update};
}
