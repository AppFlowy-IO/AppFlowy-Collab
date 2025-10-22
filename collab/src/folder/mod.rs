#![allow(clippy::module_inception)]

pub use crate::entity::define::ViewId;
pub use entities::*;
pub use folder::*;
pub use folder_migration::*;
pub use folder_observe::*;
pub use relation::*;
pub use section::*;
pub use space_info::*;
pub use view::*;
pub use workspace::*;

mod entities;
pub mod error;
mod folder;
pub mod folder_diff;
mod folder_migration;
mod folder_observe;
pub mod hierarchy_builder;
mod relation;
mod revision;
mod section;
pub mod space_info;
mod view;
mod workspace;
