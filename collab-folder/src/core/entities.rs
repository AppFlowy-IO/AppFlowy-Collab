use serde::{Deserialize, Serialize};
use serde_repr::*;

pub struct Workspace {
    pub id: String,
    pub name: String,
    pub belongings: Belongings,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct View {
    pub id: String,
    // bid short for belong to id
    pub bid: Option<String>,
    pub name: String,
    pub desc: String,
    pub belongings: Belongings,
    pub created_at: i64,
    pub layout: u8,
}

#[derive(Serialize, Deserialize)]
#[repr(transparent)]
pub struct Belongings {
    pub view_ids: Vec<String>,
}

impl Belongings {
    pub fn new() -> Self {
        Self { view_ids: vec![] }
    }

    pub fn into_inner(self) -> Vec<String> {
        self.view_ids
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ViewLayout {
    Document = 0,
    Grid = 1,
    Board = 2,
    Calendar = 3,
}
