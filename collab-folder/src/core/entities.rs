use crate::core::{View, Workspace};
use serde::{Deserialize, Serialize};
use serde_repr::*;
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

pub struct FolderData {
    pub current_workspace: String,
    pub current_view: String,
    pub workspaces: Vec<Workspace>,
    pub views: Vec<View>,
}
