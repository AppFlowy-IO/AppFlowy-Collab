#[macro_export]
macro_rules! if_native {
    ($($item:item)*) => {$(
        #[cfg(not(target_arch = "wasm32"))]
        $item
    )*}
}

#[macro_export]
macro_rules! if_wasm {
    ($($item:item)*) => {$(
        #[cfg(target_arch = "wasm32")]
        $item
    )*}
}

pub mod core;
pub mod entity;
pub mod error;
pub mod util;

pub mod preclude {
  pub use serde_json::value::Value as JsonValue;
  pub use yrs::block::Prelim;
  pub use yrs::types::{
    array::Array, Attrs, Delta as YrsDelta, EntryChange, GetString, Observable, ToJson,
    Value as YrsValue, *,
  };
  pub use yrs::*;

  pub use crate::core::collab::{Collab, CollabBuilder};
  pub use crate::core::collab_plugin::CollabPlugin;
  pub use crate::util::MapExt;
}
