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

mod any_mut;
pub mod core;
pub mod entity;
pub mod error;
pub mod lock;
pub mod util;

pub mod preclude {
  pub use serde_json::value::Value as JsonValue;
  pub use yrs::In as YrsInput;
  pub use yrs::Out as YrsValue;
  pub use yrs::block::ClientID;
  pub use yrs::block::Prelim;
  pub use yrs::types::{
    AsPrelim, Attrs, Delta as YrsDelta, EntryChange, GetString, Observable, ToJson, array::Array, *,
  };
  pub use yrs::*;

  pub use crate::any_mut::AnyMut;
  pub use crate::core::collab::Collab;
  pub use crate::core::collab_plugin::CollabPlugin;
  pub use crate::core::fill::{FillError, FillRef};
  pub use crate::util::MapExt;
  pub use crate::util::deserialize_i32_from_numeric;
  pub use crate::util::deserialize_i64_from_numeric;
}
