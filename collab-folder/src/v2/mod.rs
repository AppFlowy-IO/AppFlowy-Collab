use crate::error::FolderError;

mod folder;
mod fractional_index;
mod lww;
mod provider;
mod view;

pub type Result<T> = std::result::Result<T, FolderError>;
