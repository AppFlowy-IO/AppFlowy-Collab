use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use collab::core::array_wrapper::ArrayRefExtension;
use collab::core::collab::MutexCollab;
use collab::core::collab_state::{SnapshotState, SyncState};
use collab::preclude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;

use crate::core::folder_observe::{TrashChangeSender, ViewChangeSender};
use crate::core::trash::{TrashArray, TrashRecord};
use crate::core::{
  subscribe_folder_change, FolderData, TrashInfo, View, ViewIdentifier, ViewRelations, ViewsMap,
  Workspace, WorkspaceMap, WorkspaceUpdate,
};

const FOLDER: &str = "folder";
const WORKSPACES: &str = "workspaces";
const VIEWS: &str = "views";
const TRASH: &str = "trash";
const META: &str = "meta";
const VIEW_RELATION: &str = "relation";
const CURRENT_WORKSPACE: &str = "current_workspace";
const CURRENT_VIEW: &str = "current_view";

pub struct FolderContext {
  pub view_change_tx: ViewChangeSender,
  pub trash_change_tx: TrashChangeSender,
}

/// The folder hierarchy is like this:
/// Folder: [workspaces: [], views: {}, trash: [], meta: {}, relation: {}]
pub struct Folder {
  inner: Arc<MutexCollab>,
  root: MapRefWrapper,
  pub workspaces: WorkspaceArray,
  /// Keep track of each view's data. It's a map from view id to view data
  pub views: Rc<ViewsMap>,
  trash: TrashArray,
  pub meta: MapRefWrapper,
  /// Subscription for folder change. Like insert a new view
  #[allow(dead_code)]
  subscription: DeepEventsSubscription,
  context: FolderContext,
}

impl Folder {
  pub fn get_or_create(collab: Arc<MutexCollab>, context: FolderContext) -> Self {
    let is_exist = {
      let collab_guard = collab.lock();
      let txn = collab_guard.transact();
      let is_exist = is_folder_exist(txn, &collab_guard);
      drop(collab_guard);
      is_exist
    };
    if is_exist {
      get_folder(collab, context)
    } else {
      create_folder(collab, context)
    }
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    let rx = self.inner.lock().subscribe_sync_state();
    WatchStream::new(rx)
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    let rx = self.inner.lock().subscribe_snapshot_state();
    WatchStream::new(rx)
  }

  pub fn reload(self) -> Self {
    get_folder(self.inner, self.context)
  }

  pub fn create_with_data(&self, data: FolderData) {
    self.root.with_transact_mut(|txn| {
      for workspace in data.workspaces {
        self.workspaces.create_workspace_with_txn(txn, workspace);
      }

      for view in data.views {
        self.views.insert_view_with_txn(txn, view);
      }

      tracing::debug!("Set current workspace: {}", data.current_workspace);
      self
        .meta
        .insert_str_with_txn(txn, CURRENT_WORKSPACE, data.current_workspace);

      tracing::debug!("Set current view: {}", data.current_view);
      self
        .meta
        .insert_str_with_txn(txn, CURRENT_VIEW, data.current_view);
    })
  }

  /// Set the current workspace id. If the workspace id is not exist, do nothing.
  pub fn set_current_workspace(&self, workspace_id: &str) {
    self.meta.with_transact_mut(|txn| {
      if self.workspaces.is_exist_with_txn(txn, workspace_id) {
        tracing::debug!("Set current workspace: {}", workspace_id);
        self
          .meta
          .insert_str_with_txn(txn, CURRENT_WORKSPACE, workspace_id);
      } else {
        tracing::error!(
          "Trying to set current workspace that is not exist: {}",
          workspace_id
        );
      }
    });
  }

  pub fn get_current_workspace(&self) -> Option<Workspace> {
    let txn = self.meta.transact();
    let workspace_id = self.meta.get_str_with_txn(&txn, CURRENT_WORKSPACE)?;
    let workspace = self.workspaces.get_workspace(&workspace_id)?;
    Some(workspace)
  }

  pub fn get_current_workspace_id(&self) -> Option<String> {
    let txn = self.meta.transact();
    self.meta.get_str_with_txn(&txn, CURRENT_WORKSPACE)
  }

  pub fn get_current_workspace_views(&self) -> Vec<Arc<View>> {
    let txn = self.meta.transact();
    self.get_current_workspace_views_with_txn(&txn)
  }

  pub fn get_current_workspace_views_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Arc<View>> {
    if let Some(workspace_id) = self.meta.get_str_with_txn(txn, CURRENT_WORKSPACE) {
      self.get_workspace_views_with_txn(txn, &workspace_id)
    } else {
      vec![]
    }
  }

  pub fn get_workspace_views(&self, workspace_id: &str) -> Vec<Arc<View>> {
    let txn = self.meta.transact();
    self.get_workspace_views_with_txn(&txn, workspace_id)
  }

  pub fn get_workspace_views_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    workspace_id: &str,
  ) -> Vec<Arc<View>> {
    if let Some(workspace) = self.workspaces.get_workspace(workspace_id) {
      let view_ids = workspace
        .child_views
        .into_inner()
        .into_iter()
        .map(|be| be.id)
        .collect::<Vec<String>>();
      self.views.get_views_with_txn(txn, &view_ids)
    } else {
      vec![]
    }
  }

  pub fn insert_view(&self, view: View) {
    if let Some(workspace_id) = self.get_current_workspace_id() {
      if view.parent_view_id == workspace_id {
        self
          .workspaces
          .update_workspace(workspace_id, |workspace_update| {
            workspace_update.add_children(vec![ViewIdentifier {
              id: view.id.clone(),
            }])
          });
      }
    }
    self.views.insert_view(view)
  }

  pub fn move_view(&self, view_id: &str, from: u32, to: u32) -> Option<Arc<View>> {
    let view = self.views.get_view(view_id)?;
    if let Some(workspace_id) = self.get_current_workspace_id() {
      if view.parent_view_id == workspace_id {
        self
          .workspaces
          .update_workspace(workspace_id, |workspace_update| {
            workspace_update.move_view(from, to);
          });
        return Some(view);
      }
    }
    self.views.move_child(&view.parent_view_id, from, to);
    Some(view)
  }

  pub fn set_current_view(&self, view_id: &str) {
    tracing::debug!("Set current view: {}", view_id);
    if view_id.is_empty() {
      tracing::warn!("🟡 Set current view with empty id");
      return;
    }

    if let Some(old_current_view) = self.get_current_view() {
      if old_current_view == view_id {
        return;
      }
    }

    self.meta.with_transact_mut(|txn| {
      self.meta.insert_with_txn(txn, CURRENT_VIEW, view_id);
    });
  }

  pub fn get_current_view(&self) -> Option<String> {
    let txn = self.meta.transact();
    self.meta.get_str_with_txn(&txn, CURRENT_VIEW)
  }

  pub fn add_trash(&self, trash_ids: Vec<String>) {
    if let Some(workspace_id) = self.get_current_workspace_id() {
      let trash = trash_ids
        .into_iter()
        .map(|trash_id| TrashRecord {
          id: trash_id,
          created_at: chrono::Utc::now().timestamp(),
          workspace_id: workspace_id.clone(),
        })
        .collect::<Vec<TrashRecord>>();
      self.trash.add_trash(trash);
    }
  }

  pub fn delete_trash(&self, trash_ids: Vec<String>) {
    self.trash.delete_trash(trash_ids);
  }

  pub fn get_all_trash(&self) -> Vec<TrashInfo> {
    self.trash.get_all_trash()
  }

  pub fn remote_all_trash(&self) {
    self.trash.clear();
  }

  pub fn to_json(&self) -> String {
    self.root.to_json()
  }

  pub fn to_json_value(&self) -> JsonValue {
    self.root.to_json_value()
  }
}

pub struct WorkspaceArray {
  container: ArrayRefWrapper,
  workspaces: RwLock<HashMap<String, WorkspaceMap>>,
  view_relations: Rc<ViewRelations>,
}

impl WorkspaceArray {
  pub fn new<T: ReadTxn>(
    txn: &T,
    array_ref: ArrayRefWrapper,
    view_relations: Rc<ViewRelations>,
  ) -> Self {
    let workspace_maps = array_ref
      .to_map_refs_with_txn(txn)
      .into_iter()
      .flat_map(|map_ref| {
        let workspace_map = WorkspaceMap::new(map_ref, view_relations.clone());
        workspace_map
          .workspace_id()
          .map(|workspace_id| (workspace_id, workspace_map))
      })
      .collect::<HashMap<String, WorkspaceMap>>();
    Self {
      container: array_ref,
      workspaces: RwLock::new(workspace_maps),
      view_relations,
    }
  }

  pub fn get_all_workspaces(&self) -> Vec<Workspace> {
    let txn = self.container.transact();
    self.get_all_workspaces_with_txn(&txn)
  }

  pub fn is_exist_with_txn<T: ReadTxn>(&self, txn: &T, workspace_id: &str) -> bool {
    let ids = self.get_all_workspace_ids_with_txn(txn);
    ids.contains(&workspace_id.to_string())
  }

  pub fn get_workspace<T: AsRef<str>>(&self, workspace_id: T) -> Option<Workspace> {
    self
      .workspaces
      .read()
      .get(workspace_id.as_ref())
      .map(|workspace_map| workspace_map.to_workspace())?
  }

  pub fn get_all_workspaces_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Workspace> {
    let map_refs = self.container.to_map_refs();
    map_refs
      .into_iter()
      .flat_map(|map_ref| {
        WorkspaceMap::new(map_ref, self.view_relations.clone()).to_workspace_with_txn(txn)
      })
      .collect::<Vec<_>>()
  }

  pub fn get_all_workspace_ids_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<String> {
    let map_refs = self.container.to_map_refs_with_txn(txn);
    map_refs
      .into_iter()
      .flat_map(|map_ref| {
        WorkspaceMap::new(map_ref, self.view_relations.clone()).to_workspace_id_with_txn(txn)
      })
      .collect::<Vec<_>>()
  }

  pub fn create_workspace(&self, workspace: Workspace) {
    self
      .container
      .with_transact_mut(|txn| self.create_workspace_with_txn(txn, workspace))
  }

  pub fn delete_workspace(&self, index: u32) {
    self.container.with_transact_mut(|txn| {
      self.container.remove_with_txn(txn, index);
    })
  }

  pub fn create_workspace_with_txn(&self, txn: &mut TransactionMut, workspace: Workspace) {
    let workspace_id = workspace.id.clone();
    let map_ref = self.container.insert_map_with_txn(txn);
    let workspace_map = WorkspaceMap::create_with_txn(
      txn,
      &map_ref,
      &workspace.id,
      self.view_relations.clone(),
      |builder| {
        let _ = builder.update(|update| {
          update
            .set_name(workspace.name)
            .set_created_at(workspace.created_at)
            .set_children(workspace.child_views);
        });

        let map_ref = MapRefWrapper::new(map_ref.clone(), self.container.collab_ctx.clone());
        WorkspaceMap::new(map_ref, self.view_relations.clone())
      },
    );

    self.workspaces.write().insert(workspace_id, workspace_map);
  }

  pub fn update_workspace<T: AsRef<str>, F>(&self, workspace_id: T, f: F)
  where
    F: FnOnce(WorkspaceUpdate),
  {
    if let Some(workspace) = self.workspaces.read().get(workspace_id.as_ref()).cloned() {
      workspace.update(f);
    }
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

fn create_folder(collab: Arc<MutexCollab>, context: FolderContext) -> Folder {
  let collab_guard = collab.lock();

  let (folder, workspaces, views, trash, meta, subscription) =
    collab_guard.with_transact_mut(|txn| {
      let mut folder = collab_guard.insert_map_with_txn(txn, FOLDER);
      let subscription = subscribe_folder_change(&mut folder, context.view_change_tx.clone());
      let workspaces = folder.insert_array_with_txn::<WorkspaceItem>(txn, WORKSPACES, vec![]);
      let views = folder.insert_map_with_txn(txn, VIEWS);
      let trash = folder.insert_array_with_txn::<TrashRecord>(txn, TRASH, vec![]);
      let meta = folder.insert_map_with_txn(txn, META);

      let view_relations = Rc::new(ViewRelations::new(
        folder.insert_map_with_txn(txn, VIEW_RELATION),
      ));
      let workspaces = WorkspaceArray::new(txn, workspaces, view_relations.clone());
      let views = Rc::new(ViewsMap::new(
        views,
        context.view_change_tx.clone(),
        view_relations,
      ));
      let trash = TrashArray::new(trash, views.clone(), context.trash_change_tx.clone());
      (folder, workspaces, views, trash, meta, subscription)
    });
  drop(collab_guard);

  Folder {
    inner: collab,
    root: folder,
    workspaces,
    views,
    trash,
    meta,
    subscription,
    context,
  }
}

fn is_folder_exist(txn: Transaction, collab: &Collab) -> bool {
  collab.get_map_with_txn(&txn, vec![FOLDER]).is_some()
}

fn get_folder(collab: Arc<MutexCollab>, context: FolderContext) -> Folder {
  let collab_guard = collab.lock();
  let txn = collab_guard.transact();
  let mut folder = collab_guard.get_map_with_txn(&txn, vec![FOLDER]).unwrap();
  let folder_sub = subscribe_folder_change(&mut folder, context.view_change_tx.clone());
  let workspaces = collab_guard
    .get_array_with_txn(&txn, vec![FOLDER, WORKSPACES])
    .unwrap();
  let views = collab_guard
    .get_map_with_txn(&txn, vec![FOLDER, VIEWS])
    .unwrap();
  let trash = collab_guard
    .get_array_with_txn(&txn, vec![FOLDER, TRASH])
    .unwrap();
  let meta = collab_guard
    .get_map_with_txn(&txn, vec![FOLDER, META])
    .unwrap();

  let children_map = collab_guard
    .get_map_with_txn(&txn, vec![FOLDER, VIEW_RELATION])
    .unwrap();
  let view_relations = Rc::new(ViewRelations::new(children_map));

  let workspaces = WorkspaceArray::new(&txn, workspaces, view_relations.clone());
  let views = Rc::new(ViewsMap::new(
    views,
    context.view_change_tx.clone(),
    view_relations,
  ));

  let trash = TrashArray::new(trash, views.clone(), context.trash_change_tx.clone());
  drop(txn);
  drop(collab_guard);
  Folder {
    inner: collab,
    root: folder,
    workspaces,
    views,
    trash,
    meta,
    subscription: folder_sub,
    context,
  }
}
