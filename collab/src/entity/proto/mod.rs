#![cfg_attr(rustfmt, rustfmt_skip)]
#[allow(clippy::all)]
pub mod collab {
  include!(concat!(env!("OUT_DIR"), "/collab.rs"));
}

pub use collab::*;
