use crate::core::trash::{TrashArray, TrashItem};
use crate::core::{FolderData, ViewChangeSender, ViewsMap, Workspace, WorkspaceMap};
use collab::preclude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FOLDER: &str = "folder";
const WORKSPACES: &str = "workspaces";
const VIEWS: &str = "views";
const TRASH: &str = "trash";
const META: &str = "meta";
const CURRENT_WORKSPACE: &str = "current_workspace";
const CURRENT_VIEW: &str = "current_view";

#[derive(Default)]
pub struct FolderContext {
    pub view_change_tx: Option<ViewChangeSender>,
}

pub struct Folder {
    inner: Collab,
    root: MapRefWrapper,
    pub workspaces: WorkspaceArray,
    pub views: ViewsMap,
    pub trash: TrashArray,
    pub meta: MapRefWrapper,
}

impl Folder {
    pub fn create(collab: Collab, context: FolderContext) -> Self {
        let (folder, workspaces, views, trash, meta) = collab.with_transact_mut(|txn| {
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

            // { FOLDER: { WORKSPACES: [], VIEWS: {:}, TRASH: [], META: {:} } }
            let meta = collab
                .get_map_with_txn(txn, vec![FOLDER, META])
                .unwrap_or_else(|| folder.insert_map_with_txn(txn, META));

            (folder, workspaces, views, trash, meta)
        });
        let workspaces = WorkspaceArray::new(workspaces);
        let views = ViewsMap::new(views, context.view_change_tx);
        let trash = TrashArray::new(trash);
        Self {
            inner: collab,
            root: folder,
            workspaces,
            views,
            trash,
            meta,
        }
    }

    pub fn create_with_data(collab: Collab, data: FolderData) {
        let this = Self::create(collab, FolderContext::default());
        this.root.with_transact_mut(|txn| {
            for workspace in data.workspaces {
                this.workspaces.create_workspace_with_txn(txn, workspace);
            }

            for view in data.views {
                this.views.insert_view_with_txn(txn, view);
            }

            this.meta
                .insert_with_txn(txn, CURRENT_WORKSPACE, data.current_workspace);

            this.meta
                .insert_with_txn(txn, CURRENT_VIEW, data.current_view);
        })
    }

    pub fn set_current_workspace(&self, workspace_id: &str) {
        self.meta.insert(CURRENT_WORKSPACE, workspace_id);
    }

    pub fn get_current_workspace(&self) -> Option<String> {
        self.meta.get_str(CURRENT_WORKSPACE)
    }

    pub fn set_current_view(&self, view_id: &str) {
        self.meta.insert(CURRENT_VIEW, view_id);
    }

    pub fn get_current_view(&self) -> Option<String> {
        self.meta.get_str(CURRENT_VIEW)
    }

    pub fn to_json(&self) -> String {
        self.root.to_json()
    }
}

pub struct WorkspaceArray {
    container: ArrayRefWrapper,
    workspace_cache: RwLock<HashMap<String, WorkspaceMap>>,
}

impl WorkspaceArray {
    pub fn new(array_ref: ArrayRefWrapper) -> Self {
        let txn = array_ref.transact();
        let workspace_maps = array_ref
            .to_map_refs_with_txn(&txn)
            .into_iter()
            .flat_map(|map_ref| {
                let workspace_map = WorkspaceMap::new(map_ref);
                match workspace_map.workspace_id() {
                    None => None,
                    Some(workspace_id) => Some((workspace_id, workspace_map)),
                }
            })
            .collect::<HashMap<String, WorkspaceMap>>();
        drop(txn);
        Self {
            container: array_ref,
            workspace_cache: RwLock::new(workspace_maps),
        }
    }

    pub fn get_all_workspaces(&self) -> Vec<Workspace> {
        let txn = self.container.transact();
        self.get_all_workspaces_with_txn(&txn)
    }

    pub fn get_workspace(&self, workspace_id: &str) -> Option<Workspace> {
        self.workspace_cache
            .read()
            .get(workspace_id)
            .map(|workspace_map| workspace_map.to_workspace())?
    }

    pub fn get_all_workspaces_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Workspace> {
        let map_refs = self.container.to_map_refs();
        map_refs
            .into_iter()
            .flat_map(|map_ref| WorkspaceMap::new(map_ref).to_workspace_with_txn(txn))
            .collect::<Vec<_>>()
    }

    pub fn create_workspace(&self, workspace: Workspace) {
        self.container
            .with_transact_mut(|txn| self.create_workspace_with_txn(txn, workspace))
    }

    pub fn delete_workspace(&self, index: u32) {
        self.container.with_transact_mut(|txn| {
            self.container.remove_with_txn(txn, index);
        })
    }

    pub fn create_workspace_with_txn(&self, txn: &mut TransactionMut, workspace: Workspace) {
        let workspace_id = workspace.id.clone();
        let map_ref = self.container.create_map_with_txn(txn);
        let workspace_map = WorkspaceMap::create_with_txn(txn, map_ref, &workspace.id, |builder| {
            builder
                .update(|update| {
                    update
                        .set_name(workspace.name)
                        .set_created_at(workspace.created_at)
                        .set_belongings(workspace.belongings);
                })
                .done()
                .unwrap()
        });

        self.workspace_cache
            .write()
            .insert(workspace_id, workspace_map);
    }

    pub fn edit_workspace(&self, workspace_id: &str) -> Option<WorkspaceMap> {
        self.workspace_cache.read().get(workspace_id).cloned()
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
