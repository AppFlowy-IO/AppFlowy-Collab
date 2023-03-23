use collab_derive::Collab;
use lib0::any::Any;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Collab, Serialize, Deserialize)]
pub struct Document {
    pub(crate) doc_id: String,
    pub(crate) name: String,
    pub(crate) created_at: i64,
    pub(crate) attributes: HashMap<String, String>,
    pub(crate) tasks: HashMap<String, TaskInfo>,
    pub(crate) owner: Owner,
}

#[derive(Collab, Default, Serialize, Deserialize)]
pub struct Owner {
    pub id: String,
    pub name: String,
    pub email: String,
    pub location: Option<String>,
}

#[derive(Debug, Collab, Default, Serialize, Deserialize)]
pub struct TaskInfo {
    pub title: String,
    pub repeated: bool,
}

impl From<TaskInfo> for Any {
    fn from(task_info: TaskInfo) -> Self {
        let a = serde_json::to_value(&task_info).unwrap();
        serde_json::from_value(a).unwrap()
    }
}
