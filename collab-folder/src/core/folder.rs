use crate::core::trash::{TrashArray, TrashItem};
use crate::core::{ViewsMap, WorkspaceMap};
use collab::preclude::*;
use serde::{Deserialize, Serialize};

const FOLDER: &str = "folder";
const WORKSPACES: &str = "workspaces";
const VIEWS: &str = "views";
const TRASH: &str = "trash";

pub struct Folder {
    inner: Collab,
    root: MapRefWrapper,
    pub workspaces: WorkspaceArray,
    pub views: ViewsMap,
    pub trash: TrashArray,
}

impl Folder {
    pub fn create(collab: Collab) -> Self {
        let (folder, workspaces, views, trash) = collab.with_transact_mut(|txn| {
            // { FOLDER: {:} }
            let folder = collab
                .get_map_with_txn(txn, vec![FOLDER])
                .unwrap_or_else(|| collab.create_map_with_txn(txn, FOLDER));

            // { FOLDER: { WORKSPACES: [] } }
            let workspaces = collab
                .get_array_with_txn(txn, vec![FOLDER, WORKSPACES])
                .unwrap_or_else(|| {
                    folder.insert_array_with_txn::<WorkspaceItem>(txn, WORKSPACES, vec![])
                });

            // { FOLDER: { WORKSPACES: [], VIEWS: {:} } }
            let views = collab
                .get_map_with_txn(txn, vec![FOLDER, VIEWS])
                .unwrap_or_else(|| folder.insert_map_with_txn(txn, VIEWS));

            // { FOLDER: { WORKSPACES: [], VIEWS: {:}, TRASH: [] } }
            let trash = collab
                .get_array_with_txn(txn, vec![FOLDER, TRASH])
                .unwrap_or_else(|| folder.insert_array_with_txn::<TrashItem>(txn, TRASH, vec![]));

            (folder, workspaces, views, trash)
        });
        let workspaces = WorkspaceArray::new(workspaces);
        let views = ViewsMap::new(views);
        let trash = TrashArray::new(trash);
        Self {
            inner: collab,
            root: folder,
            workspaces,
            views,
            trash,
        }
    }

    pub fn get_workspaces(&self) -> Vec<WorkspaceItem> {
        self.workspaces.get_all_workspaces()
    }

    pub fn get_workspace_map(&self, workspace_id: &str) -> Option<WorkspaceMap> {
        let workspace_map = self.root.with_transact_mut(|txn| {
            self.root
                .get_map_with_txn(txn, workspace_id)
                .unwrap_or_else(|| self.root.insert_map_with_txn(txn, workspace_id))
        });

        Some(WorkspaceMap::new(workspace_map))
    }
}

pub struct WorkspaceArray {
    inner: ArrayRefWrapper,
}

impl WorkspaceArray {
    pub fn new(array_ref: ArrayRefWrapper) -> Self {
        Self { inner: array_ref }
    }

    pub fn get_all_workspaces(&self) -> Vec<WorkspaceItem> {
        let txn = self.inner.transact();
        self.inner
            .iter(&txn)
            .flat_map(|item| {
                if let YrsValue::Any(any) = item {
                    Some(WorkspaceItem::from(any))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Serialize, Deserialize)]
pub struct WorkspaceItem {
    workspace_id: String,
    name: String,
}

impl From<lib0Any> for WorkspaceItem {
    fn from(any: lib0Any) -> Self {
        let mut json = String::new();
        any.to_json(&mut json);
        serde_json::from_str(&json).unwrap()
    }
}

impl From<WorkspaceItem> for lib0Any {
    fn from(item: WorkspaceItem) -> Self {
        let json = serde_json::to_string(&item).unwrap();
        lib0Any::from_json(&json).unwrap()
    }
}
