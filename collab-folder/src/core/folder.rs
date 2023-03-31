use crate::core::trash::{TrashArray, TrashRecord};
use crate::core::{
  Belonging, BelongingMap, FolderData, TrashChangeSender, View, ViewChangeSender, ViewsMap,
  Workspace, WorkspaceMap,
};
use collab::preclude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

const FOLDER: &str = "folder";
const WORKSPACES: &str = "workspaces";
const VIEWS: &str = "views";
const TRASH: &str = "trash";
const META: &str = "meta";
const BELONGINGS: &str = "Belongings";
const CURRENT_WORKSPACE: &str = "current_workspace";
const CURRENT_VIEW: &str = "current_view";

#[derive(Default)]
pub struct FolderContext {
  pub view_change_tx: Option<ViewChangeSender>,
  pub trash_change_tx: Option<TrashChangeSender>,
}

pub struct Folder {
  inner: Collab,
  root: MapRefWrapper,
  pub workspaces: WorkspaceArray,
  pub views: Rc<ViewsMap>,
  pub belongings: Rc<BelongingMap>,
  pub trash: TrashArray,
  pub meta: MapRefWrapper,
}

impl Folder {
  pub fn create(collab: Collab, context: FolderContext) -> Self {
    let (folder, workspaces, views, trash, meta, belongings) = collab.with_transact_mut(|txn| {
      // { FOLDER: {:} }
      let folder = collab
        .get_map_with_txn(txn, vec![FOLDER])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, FOLDER));

      // { FOLDER: { WORKSPACES: [] } }
      let workspaces = collab
        .get_array_with_txn(txn, vec![FOLDER, WORKSPACES])
        .unwrap_or_else(|| folder.insert_array_with_txn::<WorkspaceItem>(txn, WORKSPACES, vec![]));

      // { FOLDER: { WORKSPACES: [], VIEWS: {:} } }
      let views = collab
        .get_map_with_txn(txn, vec![FOLDER, VIEWS])
        .unwrap_or_else(|| folder.insert_map_with_txn(txn, VIEWS));

      // { FOLDER: { WORKSPACES: [], VIEWS: {:}, TRASH: [] } }
      let trash = collab
        .get_array_with_txn(txn, vec![FOLDER, TRASH])
        .unwrap_or_else(|| folder.insert_array_with_txn::<TrashRecord>(txn, TRASH, vec![]));

      // { FOLDER: { WORKSPACES: [], VIEWS: {:}, TRASH: [], META: {:} } }
      let meta = collab
        .get_map_with_txn(txn, vec![FOLDER, META])
        .unwrap_or_else(|| folder.insert_map_with_txn(txn, META));

      // { FOLDER: { WORKSPACES: [], VIEWS: {:}, TRASH: [], META: {:}, BELONGINGS:{:} } }
      let belongings = collab
        .get_map_with_txn(txn, vec![FOLDER, BELONGINGS])
        .unwrap_or_else(|| folder.insert_map_with_txn(txn, BELONGINGS));

      (folder, workspaces, views, trash, meta, belongings)
    });
    let belongings = Rc::new(BelongingMap::new(belongings));
    let workspaces = WorkspaceArray::new(workspaces, belongings.clone());
    let views = Rc::new(ViewsMap::new(
      views,
      context.view_change_tx,
      belongings.clone(),
    ));
    let trash = TrashArray::new(trash, views.clone(), context.trash_change_tx);
    Self {
      inner: collab,
      root: folder,
      workspaces,
      views,
      trash,
      meta,
      belongings,
    }
  }

  pub fn add_plugins(&mut self, plugins: Vec<Arc<dyn CollabPlugin>>) {
    self.inner.add_plugins(plugins);
  }

  pub fn initial(&self) {
    self.inner.initial();
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
        .insert_with_txn(txn, CURRENT_WORKSPACE, data.current_workspace);

      tracing::debug!("Set current view: {}", data.current_view);
      self
        .meta
        .insert_with_txn(txn, CURRENT_VIEW, data.current_view);
    })
  }

  pub fn set_current_workspace(&self, workspace_id: &str) {
    tracing::debug!("Set current workspace: {}", workspace_id);
    self.meta.insert(CURRENT_WORKSPACE, workspace_id);
  }

  pub fn get_current_workspace(&self) -> Option<Workspace> {
    let workspace_id = self.meta.get_str(CURRENT_WORKSPACE)?;
    self.workspaces.get_workspace(&workspace_id)
  }

  pub fn get_current_workspace_id(&self) -> Option<String> {
    self.meta.get_str(CURRENT_WORKSPACE)
  }

  pub fn get_views_belong_to_current_workspace(&self) -> Vec<View> {
    if let Some(workspace_id) = self.meta.get_str(CURRENT_WORKSPACE) {
      if let Some(workspace) = self.workspaces.get_workspace(workspace_id) {
        let view_ids = workspace
          .belongings
          .into_inner()
          .into_iter()
          .map(|be| be.id)
          .collect::<Vec<String>>();
        return self.views.get_views(&view_ids);
      }
    }
    vec![]
  }

  pub fn insert_view(&self, view: View) {
    if let Some(workspace_id) = self.meta.get_str(CURRENT_WORKSPACE) {
      if view.bid == workspace_id {
        if let Some(workspace_map) = self.workspaces.edit_workspace(workspace_id) {
          workspace_map.update(|update| {
            update.add_belongings(vec![Belonging {
              id: view.id.clone(),
              name: view.name.clone(),
            }])
          });
        }
      }
    }
    self.views.insert_view(view)
  }

  pub fn move_view(&self, view_id: &str, from: u32, to: u32) -> Option<View> {
    let view = self.views.get_view(view_id)?;
    self.belongings.move_belonging(&view.bid, from, to);
    Some(view)
  }

  pub fn set_current_view(&self, view_id: &str) {
    tracing::debug!("Set current view: {}", view_id);
    self.meta.insert(CURRENT_VIEW, view_id);
  }

  pub fn get_current_view(&self) -> Option<String> {
    self.meta.get_str(CURRENT_VIEW)
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
  belongings: Rc<BelongingMap>,
}

impl WorkspaceArray {
  pub fn new(array_ref: ArrayRefWrapper, belongings: Rc<BelongingMap>) -> Self {
    let txn = array_ref.transact();
    let workspace_maps = array_ref
      .to_map_refs_with_txn(&txn)
      .into_iter()
      .flat_map(|map_ref| {
        let workspace_map = WorkspaceMap::new(map_ref, belongings.clone());
        workspace_map
          .workspace_id()
          .map(|workspace_id| (workspace_id, workspace_map))
      })
      .collect::<HashMap<String, WorkspaceMap>>();
    drop(txn);
    Self {
      container: array_ref,
      workspaces: RwLock::new(workspace_maps),
      belongings,
    }
  }

  pub fn get_all_workspaces(&self) -> Vec<Workspace> {
    let txn = self.container.transact();
    self.get_all_workspaces_with_txn(&txn)
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
        WorkspaceMap::new(map_ref, self.belongings.clone()).to_workspace_with_txn(txn)
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
      map_ref,
      &workspace.id,
      self.belongings.clone(),
      |builder| {
        builder
          .update(|update| {
            update
              .set_name(workspace.name)
              .set_created_at(workspace.created_at)
              .set_belongings(workspace.belongings);
          })
          .done()
          .unwrap()
      },
    );

    self.workspaces.write().insert(workspace_id, workspace_map);
  }

  pub fn edit_workspace<T: AsRef<str>>(&self, workspace_id: T) -> Option<WorkspaceMap> {
    self.workspaces.read().get(workspace_id.as_ref()).cloned()
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
