use crate::core::{View, Workspace};
use collab::preclude::lib0Any;
use serde::{Deserialize, Serialize};

use std::ops::Deref;

#[derive(Serialize, Deserialize, Default, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct Belongings {
    pub view_ids: Vec<String>,
}

impl Belongings {
    pub fn new(view_ids: Vec<String>) -> Self {
        Self { view_ids }
    }

    pub fn into_inner(self) -> Vec<String> {
        self.view_ids
    }
}

impl Deref for Belongings {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.view_ids
    }
}

impl From<Belongings> for Vec<lib0Any> {
    fn from(values: Belongings) -> Self {
        values
            .into_inner()
            .into_iter()
            .map(|value| value.into())
            .collect::<Vec<_>>()
    }
}

pub struct FolderData {
    pub current_workspace: String,
    pub current_view: String,
    pub workspaces: Vec<Workspace>,
    pub views: Vec<View>,
}

pub struct TrashInfo {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}
