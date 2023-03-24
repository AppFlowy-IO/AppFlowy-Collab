use collab::preclude::{lib0Any, ArrayRefWrapper};
use serde::{Deserialize, Serialize};

pub struct TrashArray {
    container: ArrayRefWrapper,
}

impl TrashArray {
    pub fn new(root: ArrayRefWrapper) -> Self {
        Self { container: root }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashItem {
    id: String,
}

impl From<lib0Any> for TrashItem {
    fn from(any: lib0Any) -> Self {
        let mut json = String::new();
        any.to_json(&mut json);
        serde_json::from_str(&json).unwrap()
    }
}

impl From<TrashItem> for lib0Any {
    fn from(item: TrashItem) -> Self {
        let json = serde_json::to_string(&item).unwrap();
        lib0Any::from_json(&json).unwrap()
    }
}
