use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use collab::core::collab::{DataSource, IndexContentReceiver};
use collab::core::collab_state::{SnapshotState, SyncState};
pub use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::*;
use collab::util::any_to_json_value;
use collab_entity::define::{FOLDER, FOLDER_META, FOLDER_WORKSPACE_ID};
use collab_entity::CollabType;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;
use tracing::error;

use crate::error::FolderError;
use crate::folder_observe::ViewChangeSender;
use crate::section::{Section, SectionItem, SectionMap, SectionOperation};
use crate::view::view_from_map_ref;
use crate::{
  impl_section_op, subscribe_folder_change, FolderData, SectionChangeSender, TrashInfo, View,
  ViewRelations, ViewsMap, Workspace,
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

const VIEWS: &str = "views";
const VIEW_RELATION: &str = "relation";
const CURRENT_VIEW: &str = "current_view";

pub(crate) const FAVORITES_V1: &str = "favorites";
const SECTION: &str = "section";

#[derive(Clone)]
pub struct FolderNotify {
  pub view_change_tx: ViewChangeSender,
  pub section_change_tx: SectionChangeSender,
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
  pub(crate) uid: UserId,
  pub inner: Arc<Mutex<Collab>>,
  pub(crate) root: MapRef,
  pub views: Rc<ViewsMap>,
  pub section: Rc<SectionMap>,
  pub(crate) meta: MapRef,
  #[allow(dead_code)]
  subscription: Subscription,
  #[allow(dead_code)]
  notifier: Option<FolderNotify>,
}

unsafe impl Send for Folder {}
unsafe impl Sync for Folder {}

impl Folder {
  pub fn open<T: Into<UserId>>(
    uid: T,
    collab: Arc<Mutex<Collab>>,
    notifier: Option<FolderNotify>,
  ) -> Result<Self, FolderError> {
    let uid = uid.into();
    let folder = open_folder(uid.clone(), collab.clone(), notifier.clone()).unwrap_or_else(|| {
      tracing::info!("Create missing attributes of folder");
      create_folder(uid, collab, notifier, None)
    });

    // When the folder is opened, the workspace id must be present.
    {
      let lock = folder.inner.lock().unwrap();
      folder
        .get_workspace_id_with_txn(&lock.transact())
        .ok_or_else(|| FolderError::NoRequiredData("missing workspace id".into()))?;
    }
    Ok(folder)
  }

  pub fn close(&self) {
    self.inner.lock().unwrap().clear_plugins();
  }

  pub fn validate(collab: &Collab) -> Result<(), FolderError> {
    CollabType::Folder
      .validate_require_data(collab)
      .map_err(|err| FolderError::NoRequiredData(err.to_string()))?;
    Ok(())
  }

  pub fn create<T: Into<UserId>>(
    uid: T,
    collab: Arc<Mutex<Collab>>,
    notifier: Option<FolderNotify>,
    initial_folder_data: FolderData,
  ) -> Self {
    create_folder(uid, collab, notifier, Some(initial_folder_data))
  }

  pub fn from_collab_doc_state<T: Into<UserId>>(
    uid: T,
    origin: CollabOrigin,
    collab_doc_state: DataSource,
    workspace_id: &str,
    plugins: Vec<Box<dyn CollabPlugin>>,
  ) -> Result<Self, FolderError> {
    let collab = Collab::new_with_source(origin, workspace_id, collab_doc_state, plugins, false)?;
    Self::open(uid, Arc::new(Mutex::new(collab)), None)
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.lock().unwrap().subscribe_sync_state()
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    self.inner.lock().unwrap().subscribe_snapshot_state()
  }

  pub fn subscribe_index_content(&self) -> IndexContentReceiver {
    self.inner.lock().unwrap().subscribe_index_content()
  }

  /// Returns the doc state and the state vector.
  pub fn encode_collab_v1(&self) -> Result<EncodedCollab, FolderError> {
    let lock = self.inner.lock().unwrap();
    lock.encode_collab_v1(|collab| {
      CollabType::Folder
        .validate_require_data(collab)
        .map_err(|err| FolderError::NoRequiredData(err.to_string()))
    })
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
  pub fn get_folder_data<T: ReadTxn>(&self, txn: &T, workspace_id: &str) -> Option<FolderData> {
    let folder_workspace_id = self.get_workspace_id_with_txn(txn)?;
    if folder_workspace_id != workspace_id {
      error!(
        "Workspace id not match when get folder data, expected: {}, actual: {}",
        workspace_id, folder_workspace_id
      );
      return None;
    }
    let workspace = Workspace::from(self.views.get_view_with_txn(txn, workspace_id)?.as_ref());
    let current_view = self.get_current_view_with_txn(txn).unwrap_or_default();
    let mut views = vec![];
    let orphan_views = self
      .views
      .get_orphan_views_with_txn(txn)
      .iter()
      .map(|view| view.as_ref().clone())
      .collect::<Vec<View>>();
    for view in self.views.get_views_belong_to_with_txn(txn, workspace_id) {
      views.extend(self.get_view_recursively_with_txn(txn, &view.id));
    }
    views.extend(orphan_views);

    let favorites = self
      .section
      .section_op(txn, Section::Favorite)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();
    let recent = self
      .section
      .section_op(txn, Section::Recent)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let trash = self
      .section
      .section_op(txn, Section::Trash)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let private = self
      .section
      .section_op(txn, Section::Private)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    Some(FolderData {
      workspace,
      current_view,
      views,
      favorites,
      recent,
      trash,
      private,
    })
  }

  /// Fetches the current workspace.
  ///
  /// This function fetches the ID of the current workspace from the meta object,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_workspace_info<T: ReadTxn>(&self, txn: &T, workspace_id: &str) -> Option<Workspace> {
    let folder_workspace_id: String = self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)?;
    if folder_workspace_id != workspace_id {
      error!("Workspace id not match when get current workspace");
      return None;
    }

    let view = self.views.get_view_with_txn(txn, &folder_workspace_id)?;
    Some(Workspace::from(view.as_ref()))
  }

  pub fn get_workspace_id(&self) -> Option<String> {
    let inner = self.inner.lock().unwrap();
    let txn = inner.transact();
    self.meta.get_with_txn(&txn, FOLDER_WORKSPACE_ID)
  }

  pub fn get_workspace_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)
  }

  pub fn move_view(&self, view_id: &str, from: u32, to: u32) -> Option<Arc<View>> {
    let mut lock = self.inner.lock().unwrap();
    let mut txn = lock.transact_mut();
    let view = self.views.get_view_with_txn(&txn, view_id)?;
    self
      .views
      .move_child(&mut txn, &view.parent_view_id, from, to);
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
    txn: &mut TransactionMut,
    view_id: &str,
    new_parent_id: &str,
    prev_view_id: Option<String>,
  ) -> Option<Arc<View>> {
    tracing::debug!("Move nested view: {}", view_id);
    let view = self.views.get_view_with_txn(txn, view_id)?;
    let current_workspace_id = self.get_workspace_id_with_txn(txn)?;
    let parent_id = view.parent_view_id.as_str();

    let new_parent_view = self.views.get_view_with_txn(txn, new_parent_id);

    // If the new parent is not a view, it must be a workspace.
    // Check if the new parent is the current workspace, as moving out of the current workspace is not supported yet.
    if new_parent_id != current_workspace_id && new_parent_view.is_none() {
      tracing::warn!("Unsupported move out current workspace: {}", view_id);
      return None;
    }

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
    Some(view)
  }

  pub fn set_current_view(&self, view_id: &str) {
    if view_id.is_empty() {
      tracing::warn!("🟡 Set current view with empty id");
      return;
    }

    let mut lock = self.inner.lock().unwrap();
    let mut txn = lock.transact_mut();
    if let Some(old_current_view) = self.get_current_view_with_txn(&txn) {
      if old_current_view == view_id {
        return;
      }
    }

    self.meta.insert(&mut txn, CURRENT_VIEW, view_id);
  }

  pub fn get_current_view(&self) -> Option<String> {
    let lock = self.inner.lock().unwrap();
    let txn = lock.transact();
    self.get_current_view_with_txn(&txn)
  }

  pub fn get_current_view_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_with_txn(txn, CURRENT_VIEW)
  }

  // Section operations
  // Favorites
  impl_section_op!(
    Section::Favorite,
    set_favorite,
    add_favorite_view_ids,
    delete_favorite_view_ids,
    get_my_favorite_sections,
    get_all_favorites_sections,
    remove_all_my_favorite_sections
  );

  // Recent
  impl_section_op!(
    Section::Recent,
    set_recent,
    add_recent_view_ids,
    delete_recent_view_ids,
    get_my_recent_sections,
    get_all_recent_sections,
    remove_all_my_recent_sections
  );

  // Trash
  impl_section_op!(
    Section::Trash,
    set_trash,
    add_trash_view_ids,
    delete_trash_view_ids,
    get_my_trash_sections,
    get_all_trash_sections,
    remove_all_my_trash_sections
  );

  // Private
  impl_section_op!(
    Section::Private,
    set_private,
    add_private_view_ids,
    delete_private_view_ids,
    get_my_private_sections,
    get_all_private_sections,
    remove_all_my_private_sections
  );

  pub fn get_my_trash_info<T: ReadTxn>(&self, txn: &T) -> Vec<TrashInfo> {
    self
      .get_my_trash_sections(txn)
      .into_iter()
      .flat_map(|section| {
        self
          .views
          .get_view_name_with_txn(txn, &section.id)
          .map(|name| TrashInfo {
            id: section.id,
            name,
            created_at: section.timestamp,
          })
      })
      .collect::<Vec<_>>()
  }

  pub fn is_view_in_section<T: ReadTxn>(&self, txn: &T, section: Section, view_id: &str) -> bool {
    if let Some(op) = self.section.section_op(txn, section) {
      op.contains_with_txn(txn, view_id)
    } else {
      false
    }
  }

  pub fn to_json(&self) -> String {
    self.to_json_value().to_string()
  }

  pub fn to_json_value(&self) -> JsonValue {
    let lock = self.inner.lock().unwrap();
    let txn = lock.transact();
    let any = self.root.to_json(&txn);
    println!("{:#?}", any);
    any_to_json_value(any).unwrap()
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
/// Create a folder with initial [FolderData] if it's provided.
/// Otherwise, create an empty folder.
fn create_folder<T: Into<UserId>>(
  uid: T,
  collab: Arc<Mutex<Collab>>,
  notifier: Option<FolderNotify>,
  folder_data: Option<FolderData>,
) -> Folder {
  let uid = uid.into();
  let mut collab_guard = collab.lock().unwrap();
  let c = &mut *collab_guard;
  let index_json_sender = c.index_json_sender.clone();
  let mut txn = c.context.transact_mut();
  // create the folder
  let mut folder = c.data.get_or_init_map(&mut txn, FOLDER);
  let subscription = subscribe_folder_change(&mut folder);

  // create the folder data
  let views = folder.get_or_init_map(&mut txn, VIEWS);
  let section = folder.get_or_init_map(&mut txn, SECTION);
  let meta = folder.get_or_init_map(&mut txn, FOLDER_META);
  let view_relations = Rc::new(ViewRelations::new(
    folder.get_or_init_map(&mut txn, VIEW_RELATION),
  ));

  let section = Rc::new(SectionMap::create(
    &mut txn,
    &uid,
    section,
    notifier
      .as_ref()
      .map(|notifier| notifier.section_change_tx.clone()),
  ));
  let views = Rc::new(ViewsMap::new(
    &uid,
    views,
    notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.clone()),
    view_relations,
    section.clone(),
    index_json_sender,
    HashMap::new(),
  ));

  if let Some(folder_data) = folder_data {
    let workspace_id = folder_data.workspace.id.clone();
    views.insert(&mut txn, folder_data.workspace.into(), None);

    for view in folder_data.views {
      views.insert(&mut txn, view, None);
    }

    meta.insert(&mut txn, FOLDER_WORKSPACE_ID, workspace_id);
    meta.insert(&mut txn, CURRENT_VIEW, folder_data.current_view);

    if let Some(fav_section) = section.section_op(&txn, Section::Favorite) {
      for (uid, sections) in folder_data.favorites {
        fav_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
      }
    }

    if let Some(trash_section) = section.section_op(&txn, Section::Trash) {
      for (uid, sections) in folder_data.trash {
        trash_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
      }
    }
  }
  drop(txn);
  drop(collab_guard);

  Folder {
    uid,
    inner: collab,
    root: folder,
    views,
    section,
    meta,
    subscription,
    notifier,
  }
}

pub fn check_folder_is_valid(collab: &Collab) -> Result<String, FolderError> {
  let txn = collab.transact();
  let meta: MapRef = collab
    .data
    .get_with_path(&txn, vec![FOLDER, FOLDER_META])
    .ok_or_else(|| FolderError::NoRequiredData("No meta data".to_string()))?;
  match meta.get_with_txn::<_, String>(&txn, FOLDER_WORKSPACE_ID) {
    None => Err(FolderError::NoRequiredData("No workspace id".to_string())),
    Some(workspace_id) => {
      if workspace_id.is_empty() {
        Err(FolderError::NoRequiredData("No workspace id".to_string()))
      } else {
        Ok(workspace_id)
      }
    },
  }
}

fn open_folder<T: Into<UserId>>(
  uid: T,
  collab: Arc<Mutex<Collab>>,
  notifier: Option<FolderNotify>,
) -> Option<Folder> {
  let uid = uid.into();
  let collab_guard = collab.lock().unwrap();
  let index_json_sender = collab_guard.index_json_sender.clone();
  let txn = collab_guard.transact();

  // create the folder
  let mut folder: MapRef = collab_guard.data.get_with_txn(&txn, FOLDER)?;
  let folder_sub = subscribe_folder_change(&mut folder);

  // create the folder collab objects
  let view_y_map: MapRef = folder.get_with_txn(&txn, VIEWS)?;
  // let trash = collab_guard.get_array_with_txn(&txn, vec![FOLDER, TRASH])?;
  let section_y_map: MapRef = folder.get_with_txn(&txn, SECTION)?;
  let meta_y_map: MapRef = folder.get_with_txn(&txn, FOLDER_META)?;
  let children_map_y_map: MapRef = folder.get_with_txn(&txn, VIEW_RELATION)?;

  let view_relations = Rc::new(ViewRelations::new(children_map_y_map));
  let section_map = Rc::new(SectionMap::new(
    &txn,
    &uid,
    section_y_map,
    notifier
      .as_ref()
      .map(|notifier| notifier.section_change_tx.clone()),
  )?);

  let all_views = get_views_from_root(&view_y_map, &uid, &view_relations, &section_map, &txn);
  let views_map = Rc::new(ViewsMap::new(
    &uid,
    view_y_map,
    notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.clone()),
    view_relations,
    section_map.clone(),
    index_json_sender,
    all_views,
  ));
  drop(txn);
  drop(collab_guard);

  let folder = Folder {
    uid,
    inner: collab,
    root: folder,
    views: views_map,
    section: section_map,
    meta: meta_y_map,
    subscription: folder_sub,
    notifier,
  };

  Some(folder)
}

fn get_views_from_root<T: ReadTxn>(
  root: &MapRef,
  _uid: &UserId,
  view_relations: &Rc<ViewRelations>,
  section_map: &Rc<SectionMap>,
  txn: &T,
) -> HashMap<String, Arc<View>> {
  root
    .iter(txn)
    .flat_map(|(key, value)| {
      if let Value::YMap(map) = value {
        view_from_map_ref(&map, txn, view_relations, section_map)
          .map(|view| (key.to_string(), Arc::new(view)))
      } else {
        None
      }
    })
    .collect()
}
