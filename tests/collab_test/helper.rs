use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    pub(crate) name: String,
    pub(crate) position: Position,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Position {
    pub(crate) title: String,
    pub(crate) level: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub(crate) name: String,
    pub(crate) owner: Owner,
    pub(crate) created_at: i64,
    #[serde(with = "indexmap::serde_seq")]
    pub(crate) attributes: IndexMap<String, String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Owner {
    pub name: String,
    pub email: String,
}
