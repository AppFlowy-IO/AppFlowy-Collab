use std::rc::Rc;
use std::sync::Arc;

use anyhow::Error;
use collab::core::collab::{CollabRawData, MutexCollab};
use collab::core::collab_plugin::EncodedCollabV1;
use collab::core::collab_state::{SnapshotState, SyncState};
pub use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;

use crate::folder_observe::{TrashChangeSender, ViewChangeSender};
use crate::section::{Section, SectionItem, SectionMap, SectionOperation};
use crate::trash::{TrashArray, TrashRecord};
use crate::{
  subscribe_folder_change, FolderData, TrashInfo, View, ViewRelations, ViewsMap, Workspace,
};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct UserId(pub(crate) String);

impl From<i64> for UserId {
  fn from(value: i64) -> Self {
    Self(value.to_string())
  }
}

impl From<&i64> for UserId {
  fn from(value: &i64) -> Self {
    Self(value.to_string())
  }
}

impl UserId {
  pub fn as_i64(&self) -> i64 {
    self.0.parse::<i64>().unwrap()
  }
}

impl AsRef<str> for UserId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

const FOLDER: &str = "folder";
const VIEWS: &str = "views";
const TRASH: &str = "trash";
const META: &str = "meta";
const VIEW_RELATION: &str = "relation";
const CURRENT_VIEW: &str = "current_view";
const CURRENT_WORKSPACE: &str = "current_workspace";

pub(crate) const FAVORITES_V1: &str = "favorites";
const SECTION: &str = "section";

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
/// Folder: [workspaces: [], views: {}, trash: [], favorites: { uid: [] }, meta: {}, relation: {}]
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
/// * `section`: An map of `SectionMap` objects, representing the favorite items in the folder.
/// * `meta`: Wrapper around the metadata map reference.
/// * `subscription`: A `DeepEventsSubscription` object, managing the subscription for folder changes, like inserting a new view.
/// * `notifier`: An optional `FolderNotify` object for notifying about changes in the folder.
pub struct Folder {
  uid: UserId,
  inner: Arc<MutexCollab>,
  pub(crate) root: MapRefWrapper,
  pub views: Rc<ViewsMap>,
  trash: TrashArray,
  section: Rc<SectionMap>,
  pub(crate) meta: MapRefWrapper,
  #[allow(dead_code)]
  subscription: DeepEventsSubscription,
  #[allow(dead_code)]
  notifier: Option<FolderNotify>,
}

impl Folder {
  pub fn open<T: Into<UserId>>(
    uid: T,
    collab: Arc<MutexCollab>,
    notifier: Option<FolderNotify>,
  ) -> Result<Self, Error> {
    let uid = uid.into();
    let folder = match open_folder(uid.clone(), collab.clone(), notifier.clone()) {
      None => {
        tracing::info!("Create missing attributes of folder");
        create_folder(uid, collab, notifier, None)
      },
      Some(folder) => folder,
    };

    // When the folder is opened, the workspace id must be present.
    folder.try_get_workspace_id()?;
    Ok(folder)
  }

  pub fn create<T: Into<UserId>>(
    uid: T,
    collab: Arc<MutexCollab>,
    notifier: Option<FolderNotify>,
    initial_folder_data: FolderData,
  ) -> Self {
    create_folder(uid, collab, notifier, Some(initial_folder_data))
  }

  pub fn from_collab_raw_data<T: Into<UserId>>(
    uid: T,
    origin: CollabOrigin,
    collab_raw_data: CollabRawData,
    workspace_id: &str,
    plugins: Vec<Arc<dyn CollabPlugin>>,
  ) -> Result<Self, Error> {
    let collab = MutexCollab::new_with_raw_data(origin, workspace_id, collab_raw_data, plugins)?;
    Self::open(uid, Arc::new(collab), None)
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.lock().subscribe_sync_state()
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    self.inner.lock().subscribe_snapshot_state()
  }

  /// Returns the doc state and the state vector.
  pub fn encode_collab_v1(&self) -> EncodedCollabV1 {
    self.inner.lock().encode_collab_v1()
  }

  pub fn update_workspace(&self, name: &str) {
    self.root.with_transact_mut(|txn| {
      let workspace_id = self.get_workspace_id_with_txn(txn);
      self
        .views
        .update_view_with_txn(&self.uid, txn, &workspace_id, |update| {
          update.set_name(name).done()
        });
    });
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
    let workspace_id = self.get_workspace_id_with_txn(&txn);
    let workspace = Workspace::from(self.views.get_view_with_txn(&txn, &workspace_id)?.as_ref());

    let current_view = self.get_current_view_with_txn(&txn).unwrap_or_default();

    let mut views = vec![];
    for view in self.get_workspace_views_with_txn(&txn, &workspace_id) {
      views.extend(self.get_view_recursively_with_txn(&txn, &view.id));
    }

    let favorites = self
      .section
      .section_op_with_txn(&txn, Section::Favorite)
      .map(|op| op.get_sections_with_txn(&txn))
      .unwrap_or_default();
    let recent = self
      .section
      .section_op_with_txn(&txn, Section::Recent)
      .map(|op| op.get_sections_with_txn(&txn))
      .unwrap_or_default();

    Some(FolderData {
      workspace,
      current_view,
      views,
      favorites,
      recent,
    })
  }

  /// Fetches the current workspace.
  ///
  /// This function fetches the ID of the current workspace from the meta object,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_current_workspace(&self) -> Option<Workspace> {
    let txn = self.meta.transact();
    let workspace_id = self.meta.get_str_with_txn(&txn, CURRENT_WORKSPACE)?;
    let view = self.views.get_view_with_txn(&txn, &workspace_id)?;
    Some(Workspace::from(view.as_ref()))
  }

  pub fn get_workspace_id(&self) -> String {
    let txn = self.meta.transact();
    self.get_workspace_id_with_txn(&txn)
  }

  pub fn get_workspace_id_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    self.meta.get_str_with_txn(txn, CURRENT_WORKSPACE).unwrap()
  }

  pub fn try_get_workspace_id(&self) -> Result<String, Error> {
    let txn = self.meta.transact();
    match self.meta.get_str_with_txn(&txn, CURRENT_WORKSPACE) {
      None => Err(anyhow::anyhow!("No workspace")),
      Some(workspace_id) => Ok(workspace_id),
    }
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
    self.views.get_views_belong_to_with_txn(txn, workspace_id)
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
    self.views.insert_view(view, index)
  }

  pub fn move_view(&self, view_id: &str, from: u32, to: u32) -> Option<Arc<View>> {
    let view = self.views.get_view(view_id)?;
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
    let current_workspace_id = self.get_workspace_id();
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
      self
        .views
        .dissociate_parent_child_with_txn(txn, parent_id, view_id);
      // associate the child with its new parent and place it after the prev_view_id. If the prev_view_id is None,
      // place it as the first child.
      self
        .views
        .associate_parent_child_with_txn(txn, new_parent_id, view_id, prev_view_id.clone());
      // Update the view's parent ID.
      self
        .views
        .update_view_with_txn(&self.uid, txn, view_id, |update| {
          update.set_bid(new_parent_id).done()
        });
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
    for fav_id in favorite_view_ids {
      self
        .views
        .update_view(&fav_id, |update| update.set_favorite(true).done());
    }
  }

  pub fn delete_favorites(&self, unfavorite_view_ids: Vec<String>) {
    for fav_id in unfavorite_view_ids {
      self
        .views
        .update_view(&fav_id, |update| update.set_favorite(false).done());
    }
  }

  pub fn get_all_favorites(&self) -> Vec<SectionItem> {
    self
      .section
      .section_op(Section::Favorite)
      .map(|op| op.get_all_section_item())
      .unwrap_or_default()
  }

  pub fn remove_all_favorites(&self) {
    if let Some(op) = self.section.section_op(Section::Favorite) {
      op.clear()
    }
  }

  // if the view_id has been marked as recent, this function will update it's timestamp
  pub fn add_recent_view_ids(&self, view_ids: Vec<String>) {
    for id in view_ids {
      self
        .views
        .update_view(&id, |update| update.set_recent(true).done());
    }
  }

  pub fn delete_recent_view_ids(&self, view_ids: Vec<String>) {
    for id in view_ids {
      self
        .views
        .update_view(&id, |update| update.set_recent(false).done());
    }
  }

  pub fn get_all_recent_sections(&self) -> Vec<SectionItem> {
    self
      .section
      .section_op(Section::Recent)
      .map(|op| op.get_all_section_item())
      .unwrap_or_default()
  }

  pub fn remove_all_recent_sections(&self) {
    if let Some(op) = self.section.section_op(Section::Recent) {
      op.clear()
    }
  }

  pub fn is_view_in_section(&self, section: Section, view_id: &str) -> bool {
    if let Some(op) = self.section.section_op(section) {
      op.contains_view_id(view_id)
    } else {
      false
    }
  }

  pub fn add_trash(&self, trash_ids: Vec<String>) {
    let trash = trash_ids
      .into_iter()
      .map(|trash_id| TrashRecord {
        id: trash_id,
        created_at: chrono::Utc::now().timestamp(),
        workspace_id: self.get_workspace_id(),
      })
      .collect::<Vec<TrashRecord>>();
    self.trash.add_trash(trash);
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

  pub fn create_section<S: Into<Section>>(&self, section: S) -> MapRefWrapper {
    self
      .root
      .with_transact_mut(|txn| self.section.create_section_with_txn(txn, section.into()))
  }

  pub fn section_op<S: Into<Section>>(&self, section: S) -> Option<SectionOperation> {
    self.section.section_op(section.into())
  }
}
/// Create a folder with initial [FolderData] if it's provided.
/// Otherwise, create an empty folder.
fn create_folder<T: Into<UserId>>(
  uid: T,
  collab: Arc<MutexCollab>,
  notifier: Option<FolderNotify>,
  folder_data: Option<FolderData>,
) -> Folder {
  let uid = uid.into();
  let collab_guard = collab.lock();
  let (folder, views, trash, section, meta, subscription) =
    collab_guard.with_origin_transact_mut(|txn| {
      // create the folder
      let mut folder = collab_guard.insert_map_with_txn_if_not_exist(txn, FOLDER);
      let subscription = subscribe_folder_change(&mut folder);

      // create the folder data
      let views = folder.create_map_with_txn_if_not_exist(txn, VIEWS);
      let trash = folder.create_array_if_not_exist_with_txn::<TrashRecord, _>(txn, TRASH, vec![]);
      let section = folder.create_map_with_txn_if_not_exist(txn, SECTION);
      let meta = folder.create_map_with_txn_if_not_exist(txn, META);
      let view_relations = Rc::new(ViewRelations::new(
        folder.create_map_with_txn_if_not_exist(txn, VIEW_RELATION),
      ));

      let section = Rc::new(SectionMap::create(txn, &uid, section));
      let views = Rc::new(ViewsMap::new(
        &uid,
        views,
        notifier
          .as_ref()
          .map(|notifier| notifier.view_change_tx.clone()),
        view_relations,
        section.clone(),
      ));
      let trash = TrashArray::new(
        trash,
        views.clone(),
        notifier
          .as_ref()
          .map(|notifier| notifier.trash_change_tx.clone()),
      );

      if let Some(folder_data) = folder_data {
        let workspace_id = folder_data.workspace.id.clone();
        views.insert_view_with_txn(txn, folder_data.workspace.into(), None);

        for view in folder_data.views {
          views.insert_view_with_txn(txn, view, None);
        }

        meta.insert_str_with_txn(txn, CURRENT_WORKSPACE, workspace_id);
        meta.insert_str_with_txn(txn, CURRENT_VIEW, folder_data.current_view);

        if let Some(fav_section) = section.section_op_with_txn(txn, Section::Favorite) {
          for (uid, sections) in folder_data.favorites {
            fav_section.add_sections_for_user_with_txn(txn, &uid, sections);
          }
        }
      }

      (folder, views, trash, section, meta, subscription)
    });
  drop(collab_guard);

  Folder {
    uid,
    inner: collab,
    root: folder,
    views,
    trash,
    section,
    meta,
    subscription,
    notifier,
  }
}

fn open_folder<T: Into<UserId>>(
  uid: T,
  collab: Arc<MutexCollab>,
  notifier: Option<FolderNotify>,
) -> Option<Folder> {
  let uid = uid.into();
  let collab_guard = collab.lock();
  let txn = collab_guard.transact();

  // create the folder
  let mut folder = collab_guard.get_map_with_txn(&txn, vec![FOLDER])?;
  let folder_sub = subscribe_folder_change(&mut folder);

  // create the folder collab objects
  let views = collab_guard.get_map_with_txn(&txn, vec![FOLDER, VIEWS])?;
  let trash = collab_guard.get_array_with_txn(&txn, vec![FOLDER, TRASH])?;
  let section = collab_guard.get_map_with_txn(&txn, vec![FOLDER, SECTION])?;
  let meta = collab_guard.get_map_with_txn(&txn, vec![FOLDER, META])?;
  let children_map = collab_guard.get_map_with_txn(&txn, vec![FOLDER, VIEW_RELATION])?;

  let view_relations = Rc::new(ViewRelations::new(children_map));
  let section_map = Rc::new(SectionMap::new(&txn, &uid, section)?);
  let views = Rc::new(ViewsMap::new(
    &uid,
    views,
    notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.clone()),
    view_relations,
    section_map.clone(),
  ));
  let trash = TrashArray::new(
    trash,
    views.clone(),
    notifier
      .as_ref()
      .map(|notifier| notifier.trash_change_tx.clone()),
  );
  drop(txn);
  drop(collab_guard);

  let folder = Folder {
    uid,
    inner: collab,
    root: folder,
    views,
    trash,
    section: section_map,
    meta,
    subscription: folder_sub,
    notifier,
  };

  Some(folder)
}
