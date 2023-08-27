use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Error;
use collab::core::array_wrapper::ArrayRefExtension;
use collab::core::collab::{CollabRawData, MutexCollab};
use collab::core::collab_state::{SnapshotState, SyncState};
pub use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;

use crate::core::{
  FolderData, subscribe_folder_change, TrashInfo, View, ViewIdentifier, ViewRelations, ViewsMap,
  Workspace, WorkspaceMap, WorkspaceUpdate,
};
use crate::core::favorites::{FavoriteRecord, FavoritesArray};
use crate::core::folder_observe::{TrashChangeSender, ViewChangeSender};
use crate::core::trash::{TrashArray, TrashRecord};

use super::FavoritesInfo;

const FOLDER: &str = "folder";
const WORKSPACES: &str = "workspaces";
const VIEWS: &str = "views";
const TRASH: &str = "trash";
const META: &str = "meta";
const VIEW_RELATION: &str = "relation";
const CURRENT_WORKSPACE: &str = "current_workspace";
const CURRENT_VIEW: &str = "current_view";
const FAVORITES: &str = "favorites";

#[derive(Clone)]
pub struct FolderNotify {
  pub view_change_tx: ViewChangeSender,
  pub trash_change_tx: TrashChangeSender,
}

/// Represents the folder hierarchy in a workspace.
///
/// The `Folder` structure organizes different aspects of a workspace into individual components
/// such as workspaces, views, trash, favorites, meta, and relation.
///
/// The folder hierarchy can be visualized as follows:
/// Folder: [workspaces: [], views: {}, trash: [], favorites: [], meta: {}, relation: {}]
///
///
/// # Fields
///
/// * `inner`: A mutex-protected shared pointer for managing access to the folder data.
/// * `root`: Wrapper around the root map reference.
/// * `workspaces`: An array of `WorkspaceArray` objects, representing different workspaces in the folder.
/// Currently, we only use one workspace to manage all the views in the folder.
/// * `views`: A shared pointer to a map (`ViewsMap`) from view id to view data, keeping track of each view's data.
/// * `trash`: An array of `TrashArray` objects, representing the trash items in the folder.
/// * `favorites`: An array of `FavoritesArray` objects, representing the favorite items in the folder.
/// * `meta`: Wrapper around the metadata map reference.
/// * `subscription`: A `DeepEventsSubscription` object, managing the subscription for folder changes, like inserting a new view.
/// * `notifier`: An optional `FolderNotify` object for notifying about changes in the folder.
pub struct Folder {
  inner: Arc<MutexCollab>,
  root: MapRefWrapper,
  pub workspaces: WorkspaceArray,
  pub views: Rc<ViewsMap>,
  trash: TrashArray,
  favorites: FavoritesArray,
  pub meta: MapRefWrapper,
  #[allow(dead_code)]
  subscription: DeepEventsSubscription,
  #[allow(dead_code)]
  notifier: Option<FolderNotify>,
}

impl Folder {
  pub fn open(collab: Arc<MutexCollab>, notifier: Option<FolderNotify>) -> Self {
    match open_folder(collab.clone(), notifier.clone()) {
      None => {
        tracing::info!("Create missing attributes of folder");
        create_folder(collab, notifier, None)
      },
      Some(folder) => folder,
    }
  }

  pub fn create(
    collab: Arc<MutexCollab>,
    notifier: Option<FolderNotify>,
    initial_folder_data: Option<FolderData>,
  ) -> Self {
    create_folder(collab, notifier, initial_folder_data)
  }

  pub fn from_collab_raw_data(
    origin: CollabOrigin,
    collab_raw_data: CollabRawData,
    workspace_id: &str,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Result<Self, Error> {
    let collab = MutexCollab::new_with_raw_data(origin, workspace_id, collab_raw_data, plugins)?;
    Ok(Self::open(Arc::new(collab), None))
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    let rx = self.inner.lock().subscribe_sync_state();
    WatchStream::new(rx)
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    let rx = self.inner.lock().subscribe_snapshot_state();
    WatchStream::new(rx)
  }
  /// Fetches the folder data based on the current workspace and view.
  ///
  /// This function initiates a transaction on the root node and uses it to fetch the current workspace
  /// and view. It also fetches all workspaces and their respective views.
  ///
  /// It goes through all the workspaces and fetches the views recursively for each workspace.
  ///
  /// # Returns
  ///
  /// * `Some(FolderData)`: If the operation is successful, it returns `Some` variant wrapping `FolderData`
  ///   object, which consists of current workspace ID, current view, a list of workspaces, and their respective views.
  ///
  /// * `None`: If the operation is unsuccessful (though it should typically not be the case as `Some`
  ///   is returned explicitly), it returns `None`.
  pub fn get_folder_data(&self) -> Option<FolderData> {
    let txn = self.root.transact();
    let current_workspace = self
      .get_current_workspace_id_with_txn(&txn)
      .unwrap_or_default();
    let current_view = self.get_current_view_with_txn(&txn).unwrap_or_default();
    let workspaces = self.workspaces.get_all_workspaces_with_txn(&txn);

    let mut views = vec![];
    for workspace in workspaces.iter() {
      for view in self.get_workspace_views_with_txn(&txn, &workspace.id) {
        views.extend(self.get_view_recursively_with_txn(&txn, &view.id));
      }
    }

    Some(FolderData {
      current_workspace_id: current_workspace,
      current_view,
      workspaces,
      views,
    })
  }

  /// Sets the current workspace.
  ///
  /// This function accepts a `workspace_id` as a string slice, which is used to
  /// identify the workspace that needs to be set as the current workspace.
  ///
  /// If the workspace with the provided ID exists, the function sets it as the current workspace.
  /// If not, it logs an error message.
  ///
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

  /// Fetches the current workspace.
  ///
  /// This function fetches the ID of the current workspace from the meta object,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_current_workspace(&self) -> Option<Workspace> {
    let txn = self.meta.transact();
    let workspace_id = self.meta.get_str_with_txn(&txn, CURRENT_WORKSPACE)?;
    let workspace = self.workspaces.get_workspace(workspace_id)?;
    Some(workspace)
  }

  pub fn get_current_workspace_id(&self) -> Option<String> {
    let txn = self.meta.transact();
    self.get_current_workspace_id_with_txn(&txn)
  }

  pub fn get_current_workspace_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_str_with_txn(txn, CURRENT_WORKSPACE)
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

  /// Fetches all views associated with a specific workspace, using a provided transaction.
  ///
  /// It uses the workspace ID to fetch the relevant workspace. Then, it gets all the child view IDs
  /// associated with this workspace and uses these IDs
  /// to fetch the actual view objects.
  ///
  /// # Parameters
  ///
  /// * `txn`: A transaction that is used to ensure the consistency of the fetched data.
  /// * `workspace_id`: A string slice that represents the ID of the workspace whose views are to be fetched.
  ///
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

  /// Inserts a new view into the workspace.
  ///
  /// The function first checks if there is a current workspace ID. If there is, it then checks
  /// if the `parent_view_id` of the new view matches the workspace ID. If they match,
  /// the new view is added to the workspace's children.
  ///
  /// Finally, the view is inserted into the view storage regardless of its parent view ID
  /// and workspace ID matching.
  ///
  /// # Parameters
  ///
  /// * `view`: The `View` object that is to be inserted into the storage.
  pub fn insert_view(&self, view: View, index: Option<u32>) {
    if let Some(workspace_id) = self.get_current_workspace_id() {
      if view.parent_view_id == workspace_id {
        self
          .workspaces
          .update_workspace(workspace_id, |workspace_update| {
            workspace_update.add_children(
              vec![ViewIdentifier {
                id: view.id.clone(),
              }],
              index,
            )
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

  /// Moves a nested view to a new location in the hierarchy.
  ///
  /// This function takes the `view_id` of the view to be moved,
  /// `new_parent_id` of the view under which the `view_id` should be moved,
  /// and an optional `new_prev_id` to position the `view_id` right after
  /// this specific view.
  ///
  /// If `new_prev_id` is provided, the moved view will be placed right after
  /// the view corresponding to `new_prev_id` under the `new_parent_id`.
  /// If `new_prev_id` is `None`, the moved view will become the first child of the new parent.
  ///
  /// # Arguments
  ///
  /// * `view_id` - A string slice that holds the id of the view to be moved.
  /// * `new_parent_id` - A string slice that holds the id of the new parent view.
  /// * `prev_view_id` - An `Option<String>` that holds the id of the view after which the `view_id` should be positioned.
  ///
  pub fn move_nested_view(
    &self,
    view_id: &str,
    new_parent_id: &str,
    prev_view_id: Option<String>,
  ) -> Option<Arc<View>> {
    tracing::debug!("Move nested view: {}", view_id);
    let view = self.views.get_view(view_id)?;
    let current_workspace_id = self.get_current_workspace_id()?;
    let parent_id = view.parent_view_id.as_str();

    let new_parent_view = self.views.get_view(new_parent_id);

    // If the new parent is not a view, it must be a workspace.
    // Check if the new parent is the current workspace, as moving out of the current workspace is not supported yet.
    if new_parent_id != current_workspace_id && new_parent_view.is_none() {
      tracing::warn!("Unsupported move out current workspace: {}", view_id);
      return None;
    }

    self.meta.with_transact_mut(|txn| {
      // dissociate the child from its parent
      if parent_id == current_workspace_id {
        self
          .workspaces
          .view_relations
          .dissociate_parent_child_with_txn(txn, parent_id, view_id);
      } else {
        self
          .views
          .dissociate_parent_child_with_txn(txn, parent_id, view_id);
      }
      // associate the child with its new parent and place it after the prev_view_id. If the prev_view_id is None,
      // place it as the first child.
      if new_parent_id == current_workspace_id {
        self
          .workspaces
          .view_relations
          .associate_parent_child_with_txn(txn, new_parent_id, view_id, prev_view_id.clone());
      } else {
        self.views.associate_parent_child_with_txn(
          txn,
          new_parent_id,
          view_id,
          prev_view_id.clone(),
        );
      }
      // Update the view's parent ID.
      self
        .views
        .update_view_with_txn(txn, view_id, |update| update.set_bid(new_parent_id).done());
    });
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
    self.get_current_view_with_txn(&txn)
  }

  pub fn get_current_view_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_str_with_txn(txn, CURRENT_VIEW)
  }

  pub fn add_favorites(&self, favorite_view_ids: Vec<String>) {
    if let Some(workspace_id) = self.get_current_workspace_id() {
      let favorite = favorite_view_ids
        .into_iter()
        .map(|favorite_view_id| FavoriteRecord {
          id: favorite_view_id,
          workspace_id: workspace_id.clone(),
        })
        .collect::<Vec<FavoriteRecord>>();
      self.favorites.add_favorites(favorite);
    }
  }

  pub fn delete_favorites(&self, unfavorite_view_ids: Vec<String>) {
    self.favorites.delete_favorites(unfavorite_view_ids);
  }

  pub fn get_all_favorites(&self) -> Vec<FavoritesInfo> {
    self.favorites.get_all_favorites()
  }

  pub fn remove_all_favorites(&self) {
    self.favorites.clear();
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

  /// Recursively retrieves all views associated with the provided `view_id` using a transaction.
  ///
  /// The function begins by attempting to retrieve the parent view associated with the `view_id`.
  /// If the parent view is not found, an empty vector is returned.
  /// If the parent view is found, the function proceeds to retrieve all of its child views recursively.
  ///
  /// The function finally returns a vector containing the parent view and all of its child views.
  /// The views are clones of the original objects.
  ///
  /// # Parameters
  ///
  /// * `txn`: A read transaction object which is used to execute the view retrieval.
  /// * `view_id`: The ID of the parent view.
  ///
  /// # Returns
  ///
  /// * `Vec<View>`: A vector of `View` objects that includes the parent view and all of its child views.
  pub fn get_view_recursively_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<View> {
    match self.views.get_view_with_txn(txn, view_id) {
      None => vec![],
      Some(parent_view) => {
        let mut views = vec![parent_view.as_ref().clone()];
        let child_views = parent_view
          .children
          .items
          .iter()
          .flat_map(|child| self.get_view_recursively_with_txn(txn, &child.id))
          .collect::<Vec<_>>();
        views.extend(child_views);
        views
      },
    }
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
          .get_workspace_id_with_txn(txn)
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
    let map_ref = self.container.insert_map_with_txn(txn, None);
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

/// Create a folder with initial [FolderData] if it's provided.
/// Otherwise, create an empty folder.
fn create_folder(
  collab: Arc<MutexCollab>,
  notifier: Option<FolderNotify>,
  folder_data: Option<FolderData>,
) -> Folder {
  let collab_guard = collab.lock();

  let (folder, workspaces, views, trash, favorites, meta, subscription) = collab_guard
    .with_origin_transact_mut(|txn| {
      // create the folder
      let mut folder = collab_guard.insert_map_with_txn_if_not_exist(txn, FOLDER);
      let subscription = subscribe_folder_change(&mut folder);

      // create the folder data
      let workspaces =
        folder.create_array_if_not_exist_with_txn::<WorkspaceItem>(txn, WORKSPACES, vec![]);
      let views = folder.create_map_with_txn_if_not_exist(txn, VIEWS);
      let trash = folder.create_array_if_not_exist_with_txn::<TrashRecord>(txn, TRASH, vec![]);
      let favorites =
        folder.create_array_if_not_exist_with_txn::<FavoriteRecord>(txn, FAVORITES, vec![]);
      let meta = folder.create_map_with_txn_if_not_exist(txn, META);
      let view_relations = Rc::new(ViewRelations::new(
        folder.create_map_with_txn_if_not_exist(txn, VIEW_RELATION),
      ));

      let workspaces = WorkspaceArray::new(txn, workspaces, view_relations.clone());
      let views = Rc::new(ViewsMap::new(
        views,
        notifier
          .as_ref()
          .map(|notifier| notifier.view_change_tx.clone()),
        view_relations,
      ));
      let trash = TrashArray::new(
        trash,
        views.clone(),
        notifier
          .as_ref()
          .map(|notifier| notifier.trash_change_tx.clone()),
      );
      let favorites = FavoritesArray::new(favorites, views.clone());

      // Insert the folder data if it's provided.
      if let Some(folder_data) = folder_data {
        debug_assert_eq!(folder_data.workspaces.len(), 1);

        for workspace in folder_data.workspaces {
          workspaces.create_workspace_with_txn(txn, workspace);
        }

        for view in folder_data.views {
          views.insert_view_with_txn(txn, view);
        }

        meta.insert_str_with_txn(txn, CURRENT_WORKSPACE, folder_data.current_workspace_id);
        meta.insert_str_with_txn(txn, CURRENT_VIEW, folder_data.current_view);
      }

      (
        folder,
        workspaces,
        views,
        trash,
        favorites,
        meta,
        subscription,
      )
    });
  drop(collab_guard);

  Folder {
    inner: collab,
    root: folder,
    workspaces,
    views,
    trash,
    favorites,
    meta,
    subscription,
    notifier,
  }
}

fn open_folder(collab: Arc<MutexCollab>, notifier: Option<FolderNotify>) -> Option<Folder> {
  let collab_guard = collab.lock();
  let txn = collab_guard.transact();

  // create the folder
  let mut folder = collab_guard.get_map_with_txn(&txn, vec![FOLDER])?;
  let folder_sub = subscribe_folder_change(&mut folder);

  // create the folder collab objects
  let workspaces = collab_guard.get_array_with_txn(&txn, vec![FOLDER, WORKSPACES])?;
  let views = collab_guard.get_map_with_txn(&txn, vec![FOLDER, VIEWS])?;
  let trash = collab_guard.get_array_with_txn(&txn, vec![FOLDER, TRASH])?;
  let favorite_array = collab_guard.get_array_with_txn(&txn, vec![FOLDER, FAVORITES])?;
  let meta = collab_guard.get_map_with_txn(&txn, vec![FOLDER, META])?;
  let children_map = collab_guard.get_map_with_txn(&txn, vec![FOLDER, VIEW_RELATION])?;

  let view_relations = Rc::new(ViewRelations::new(children_map));
  let workspaces = WorkspaceArray::new(&txn, workspaces, view_relations.clone());
  let views = Rc::new(ViewsMap::new(
    views,
    notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.clone()),
    view_relations,
  ));
  let favorites = FavoritesArray::new(favorite_array, views.clone());
  let trash = TrashArray::new(
    trash,
    views.clone(),
    notifier
      .as_ref()
      .map(|notifier| notifier.trash_change_tx.clone()),
  );
  drop(txn);
  drop(collab_guard);

  Some(Folder {
    inner: collab,
    root: folder,
    workspaces,
    views,
    trash,
    favorites,
    meta,
    subscription: folder_sub,
    notifier,
  })
}
