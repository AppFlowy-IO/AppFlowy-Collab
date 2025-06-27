use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use collab::core::collab::{CollabOptions, DataSource, IndexContentSender};
pub use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::*;
use collab::util::any_to_json_value;
use collab_entity::CollabType;
use collab_entity::define::{FOLDER, FOLDER_META, FOLDER_WORKSPACE_ID};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::error::FolderError;
use crate::folder_observe::ViewChangeSender;
use crate::hierarchy_builder::{FlattedViews, ParentChildViews};
use crate::section::{Section, SectionItem, SectionMap};
use crate::view::view_from_map_ref;
use crate::{
  FolderData, ParentChildRelations, SectionChangeSender, SpacePermission, TrashInfo, View,
  ViewUpdate, ViewsMap, Workspace, impl_section_op, subscribe_folder_change,
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
const PARENT_CHILD_VIEW_RELATION: &str = "relation";
const CURRENT_VIEW: &str = "current_view";
const CURRENT_VIEW_FOR_USER: &str = "current_view_for_user";

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
///   Currently, we only use one workspace to manage all the views in the folder.
/// * `views`: A shared pointer to a map (`ViewsMap`) from view id to view data, keeping track of each view's data.
/// * `trash`: An array of `TrashArray` objects, representing the trash items in the folder.
/// * `section`: An map of `SectionMap` objects, representing the favorite items in the folder.
/// * `meta`: Wrapper around the metadata map reference.
/// * `subscription`: A `DeepEventsSubscription` object, managing the subscription for folder changes, like inserting a new view.
/// * `notifier`: An optional `FolderNotify` object for notifying about changes in the folder.
pub struct Folder {
  pub collab: Collab,
  pub body: FolderBody,
}

impl Folder {
  pub fn open(mut collab: Collab, notifier: Option<FolderNotify>) -> Result<Self, FolderError> {
    let body = FolderBody::open(&mut collab, notifier)?;
    let folder = Folder { collab, body };
    if folder.get_workspace_id().is_none() {
      // When the folder is opened, the workspace id must be present.
      Err(FolderError::NoRequiredData("missing workspace id".into()))
    } else {
      Ok(folder)
    }
  }

  pub fn create(mut collab: Collab, notifier: Option<FolderNotify>, data: FolderData) -> Self {
    let body = FolderBody::open_with(&mut collab, notifier, Some(data));
    Folder { collab, body }
  }

  pub fn from_collab_doc_state(
    origin: CollabOrigin,
    collab_doc_state: DataSource,
    workspace_id: &str,
    client_id: ClientID,
  ) -> Result<Self, FolderError> {
    let options =
      CollabOptions::new(workspace_id.to_string(), client_id).with_data_source(collab_doc_state);
    let collab = Collab::new_with_options(origin, options)?;
    Self::open(collab, None)
  }

  pub fn close(&self) {
    self.collab.remove_all_plugins();
  }

  pub fn validate(&self) -> Result<(), FolderError> {
    CollabType::Folder
      .validate_require_data(&self.collab)
      .map_err(|err| FolderError::NoRequiredData(err.to_string()))?;
    Ok(())
  }

  /// Returns the doc state and the state vector.
  pub fn encode_collab(&self) -> Result<EncodedCollab, FolderError> {
    self.collab.encode_collab_v1(|collab| {
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
  pub fn get_folder_data(&self, workspace_id: &str, uid: i64) -> Option<FolderData> {
    let txn = self.collab.transact();
    self.body.get_folder_data(&txn, workspace_id, uid)
  }

  pub async fn subscribe_view_change(&self, uid: i64) -> Result<(), FolderError> {
    let txn = self.collab.transact();
    let index_json_sender = self.collab.index_json_sender.clone();
    self
      .body
      .subscribe_view_change(uid, index_json_sender, txn)
      .await;
    Ok(())
  }

  /// Fetches the current workspace.
  ///
  /// This function fetches the ID of the current workspace from the meta object,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_workspace_info(&self, workspace_id: &str, uid: i64) -> Option<Workspace> {
    let txn = self.collab.transact();
    self.body.get_workspace_info(&txn, workspace_id, uid)
  }

  pub fn get_workspace_id(&self) -> Option<String> {
    let txn = self.collab.transact();
    self.body.get_workspace_id(&txn)
  }

  pub fn get_all_views(&self, uid: i64) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_all_views(&txn, uid)
  }

  pub fn get_views<T: AsRef<str>>(&self, view_ids: &[T], uid: i64) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_views(&txn, view_ids, uid)
  }

  pub fn get_views_belong_to(&self, parent_id: &str, uid: i64) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_views_belong_to(&txn, parent_id, uid)
  }

  pub fn move_view(&mut self, view_id: &str, from: u32, to: u32, uid: i64) -> Option<Arc<View>> {
    let mut txn = self.collab.transact_mut();
    self.body.move_view(&mut txn, view_id, from, to, uid)
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
    &mut self,
    view_id: &str,
    new_parent_id: &str,
    prev_view_id: Option<String>,
    uid: i64,
  ) -> Option<Arc<View>> {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .move_nested_view(&mut txn, view_id, new_parent_id, prev_view_id, uid)
  }

  pub fn set_current_view(&mut self, view_id: String, uid: i64) {
    let mut txn = self.collab.transact_mut();
    self.body.set_current_view(&mut txn, view_id, uid);
  }

  pub fn get_current_view(&self, uid: i64) -> Option<String> {
    let txn = self.collab.transact();
    self.body.get_current_view(&txn, uid)
  }

  pub fn update_view<F>(&mut self, view_id: &str, f: F, uid: i64) -> Option<Arc<View>>
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    let mut txn = self.collab.transact_mut();
    self.body.views.update_view(&mut txn, view_id, f, uid)
  }

  pub fn delete_views<T: AsRef<str>>(&mut self, views: Vec<T>) {
    let mut txn = self.collab.transact_mut();
    self.body.views.delete_views(&mut txn, views);
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
    remove_all_my_favorite_sections,
    move_favorite_view_id
  );

  // Recent
  impl_section_op!(
    Section::Recent,
    set_recent,
    add_recent_view_ids,
    delete_recent_view_ids,
    get_my_recent_sections,
    get_all_recent_sections,
    remove_all_my_recent_sections,
    move_recent_view_id
  );

  // Trash
  impl_section_op!(
    Section::Trash,
    set_trash,
    add_trash_view_ids,
    delete_trash_view_ids,
    get_my_trash_sections,
    get_all_trash_sections,
    remove_all_my_trash_sections,
    move_trash_view_id
  );

  // Private
  impl_section_op!(
    Section::Private,
    set_private,
    add_private_view_ids,
    delete_private_view_ids,
    get_my_private_sections,
    get_all_private_sections,
    remove_all_my_private_sections,
    move_private_view_id
  );

  pub fn get_my_trash_info(&self, uid: i64) -> Vec<TrashInfo> {
    let txn = self.collab.transact();
    self
      .get_my_trash_sections(uid)
      .into_iter()
      .flat_map(|section| {
        self
          .body
          .views
          .get_view_name_with_txn(&txn, &section.id)
          .map(|name| TrashInfo {
            id: section.id,
            name,
            created_at: section.timestamp,
          })
      })
      .collect()
  }

  /// Inserts a new view into the specified workspace under a given parent view.
  ///
  /// # Parameters:
  /// - `parent_view_id`: The ID of the parent view under which the new view will be added.
  /// - `index`: Optional. If provided, the new view will be inserted at the specified position
  ///    among the parent view's children. If `None`, the new view will be added at the end of
  ///    the children list.
  ///
  /// # Behavior:
  /// - When `index` is `Some`, the new view is inserted at that position in the list of the
  ///   parent view's children.
  /// - When `index` is `None`, the new view is appended to the end of the parent view's children.
  ///
  /// Represents a view that serves as an identifier for a specific [`Collab`] object.
  /// A view can represent different types of [`Collab`] objects, such as a document or a database.
  /// When a view is inserted, its id is the[`Collab`] object id.
  ///
  pub fn insert_view(&mut self, view: View, index: Option<u32>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    self.body.views.insert(&mut txn, view, index, uid);
  }

  /// Insert a list of views at the end of its parent view
  pub fn insert_views(&mut self, views: Vec<View>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for view in views {
      self.body.views.insert(&mut txn, view, None, uid);
    }
  }

  /// Insert parent-children views into the folder.
  /// when only insert one view, user [Self::insert_view] instead.
  pub fn insert_nested_views(&mut self, views: Vec<ParentChildViews>, uid: i64) {
    let views = FlattedViews::flatten_views(views);
    let mut txn = self.collab.transact_mut();
    for view in views {
      self.body.views.insert(&mut txn, view, None, uid);
    }
  }

  pub fn get_view(&self, view_id: &str, uid: i64) -> Option<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_view(&txn, view_id, uid)
  }

  pub fn is_view_in_section(&self, section: Section, view_id: &str, uid: i64) -> bool {
    let txn = self.collab.transact();
    if let Some(op) = self.body.section.section_op(&txn, section, uid) {
      op.contains_with_txn(&txn, view_id)
    } else {
      false
    }
  }

  pub fn to_json(&self) -> String {
    self.to_json_value().to_string()
  }

  pub fn to_json_value(&self) -> JsonValue {
    let txn = self.collab.transact();
    let any = self.body.root.to_json(&txn);
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
  pub fn get_view_recursively(&self, view_id: &str, uid: i64) -> Vec<View> {
    let txn = self.collab.transact();
    let mut views = vec![];
    self.body.get_view_recursively_with_txn(
      &txn,
      view_id,
      &mut HashSet::default(),
      &mut views,
      uid,
    );
    views
  }
}

impl Deref for Folder {
  type Target = Collab;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}

impl DerefMut for Folder {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.collab
  }
}

impl Borrow<Collab> for Folder {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for Folder {
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
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

fn get_views_from_root<T: ReadTxn>(
  root: &MapRef,
  view_relations: &Arc<ParentChildRelations>,
  section_map: &Arc<SectionMap>,
  txn: &T,
  uid: i64,
) -> HashMap<String, Arc<View>> {
  root
    .iter(txn)
    .flat_map(|(key, value)| {
      if let YrsValue::YMap(map) = value {
        view_from_map_ref(&map, txn, view_relations, section_map, uid)
          .map(|view| (key.to_string(), Arc::new(view)))
      } else {
        None
      }
    })
    .collect()
}

pub struct FolderBody {
  pub root: MapRef,
  pub views: Arc<ViewsMap>,
  pub section: Arc<SectionMap>,
  pub meta: MapRef,
  #[allow(dead_code)]
  subscription: Subscription,
  #[allow(dead_code)]
  notifier: Option<FolderNotify>,
}

impl FolderBody {
  pub fn open(collab: &mut Collab, notifier: Option<FolderNotify>) -> Result<Self, FolderError> {
    CollabType::Folder.validate_require_data(collab)?;
    Ok(Self::open_with(collab, notifier, None))
  }

  pub fn open_with(
    collab: &mut Collab,
    notifier: Option<FolderNotify>,
    folder_data: Option<FolderData>,
  ) -> Self {
    let mut txn = collab.context.transact_mut();
    // create the folder
    let mut folder = collab.data.get_or_init_map(&mut txn, FOLDER);
    let subscription = subscribe_folder_change(&mut folder);

    // create the folder data
    let views: MapRef = folder.get_or_init(&mut txn, VIEWS);
    let section: MapRef = folder.get_or_init(&mut txn, SECTION);
    let meta: MapRef = folder.get_or_init(&mut txn, FOLDER_META);
    let parent_child_relations = Arc::new(ParentChildRelations::new(
      folder.get_or_init(&mut txn, PARENT_CHILD_VIEW_RELATION),
    ));

    let section = Arc::new(SectionMap::create(
      &mut txn,
      section,
      notifier
        .as_ref()
        .map(|notifier| notifier.section_change_tx.clone()),
    ));
    let views = Arc::new(ViewsMap::new(
      views,
      notifier
        .as_ref()
        .map(|notifier| notifier.view_change_tx.clone()),
      parent_child_relations,
      section.clone(),
    ));

    if let Some(folder_data) = folder_data {
      let workspace_id = folder_data.workspace.id.clone();
      views.insert(
        &mut txn,
        folder_data.workspace.into(),
        None,
        folder_data.uid,
      );

      for view in folder_data.views {
        views.insert(&mut txn, view, None, folder_data.uid);
      }

      meta.insert(&mut txn, FOLDER_WORKSPACE_ID, workspace_id);
      // For compatibility with older collab library which doesn't use CURRENT_VIEW_FOR_USER.
      meta.insert(&mut txn, CURRENT_VIEW, folder_data.current_view.clone());
      let current_view_for_user = meta.get_or_init_map(&mut txn, CURRENT_VIEW_FOR_USER);
      current_view_for_user.insert(
        &mut txn,
        folder_data.uid.to_string(),
        folder_data.current_view.clone(),
      );

      if let Some(fav_section) = section.section_op(&txn, Section::Favorite, folder_data.uid) {
        for (uid, sections) in folder_data.favorites {
          fav_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
        }
      }

      if let Some(trash_section) = section.section_op(&txn, Section::Trash, folder_data.uid) {
        for (uid, sections) in folder_data.trash {
          trash_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
        }
      }
    }
    Self {
      root: folder,
      views,
      section,
      meta,
      subscription,
      notifier,
    }
  }

  pub async fn subscribe_view_change<T: ReadTxn>(
    &self,
    uid: i64,
    index_json_sender: IndexContentSender,
    txn: T,
  ) {
    let all_views = get_views_from_root(
      &self.root,
      &self.views.parent_children_relation,
      &self.section,
      &txn,
      uid,
    );
    self
      .views
      .subscribe_view_change(uid, all_views, index_json_sender)
      .await;
  }

  pub fn get_workspace_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)
  }

  /// Recursively retrieves all views associated with the provided `view_id` using a transaction,
  /// adding them to the `accumulated_views` vector.
  ///
  /// The function begins by attempting to retrieve the view associated with the `view_id`.
  /// If the parent view is not found, the function returns.
  /// If the parent view is found, the function proceeds to retrieve all of its child views recursively.
  /// The function uses a hash set to keep track of the visited view ids to avoid infinite recursion due
  /// to circular dependency.
  ///
  /// At the end of the recursion, `accumulated_views` will contain the parent view and all of its child views.
  /// The views are clones of the original objects.
  ///
  /// # Parameters
  ///
  /// * `txn`: A read transaction object which is used to execute the view retrieval.
  /// * `view_id`: The ID of the parent view.
  /// * `visited`: Hash set containing all the traversed view ids.
  /// * `accumulated_views`: Vector containing all the views that are accumulated during the traversal.
  pub fn get_view_recursively_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    visited: &mut HashSet<String>,
    accumulated_views: &mut Vec<View>,
    uid: i64,
  ) {
    if !visited.insert(view_id.to_string()) {
      return;
    }
    match self.views.get_view_with_txn(txn, view_id, uid) {
      None => (),
      Some(parent_view) => {
        accumulated_views.push(parent_view.as_ref().clone());
        parent_view.children.items.iter().for_each(|child| {
          self.get_view_recursively_with_txn(txn, &child.id, visited, accumulated_views, uid)
        })
      },
    }
  }

  pub fn get_workspace_info<T: ReadTxn>(
    &self,
    txn: &T,
    workspace_id: &str,
    uid: i64,
  ) -> Option<Workspace> {
    let folder_workspace_id: String = self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)?;
    if folder_workspace_id != workspace_id {
      error!("Workspace id not match when get current workspace");
      return None;
    }

    let view = self
      .views
      .get_view_with_txn(txn, &folder_workspace_id, uid)?;
    Some(Workspace::from(view.as_ref()))
  }

  pub fn get_folder_data<T: ReadTxn>(
    &self,
    txn: &T,
    workspace_id: &str,
    uid: i64,
  ) -> Option<FolderData> {
    let folder_workspace_id = self.get_workspace_id_with_txn(txn)?;
    if folder_workspace_id != workspace_id {
      error!(
        "Workspace id not match when get folder data, expected: {}, actual: {}",
        workspace_id, folder_workspace_id
      );
      return None;
    }
    let workspace = Workspace::from(
      self
        .views
        .get_view_with_txn(txn, workspace_id, uid)?
        .as_ref(),
    );
    let current_view = self.get_current_view(txn, uid).unwrap_or_default();
    let mut views = vec![];
    let orphan_views = self
      .views
      .get_orphan_views_with_txn(txn, uid)
      .iter()
      .map(|view| view.as_ref().clone())
      .collect::<Vec<View>>();
    for view in self.views.get_views_belong_to(txn, workspace_id, uid) {
      let mut all_views_in_workspace = vec![];
      self.get_view_recursively_with_txn(
        txn,
        &view.id,
        &mut HashSet::default(),
        &mut all_views_in_workspace,
        uid,
      );
      views.extend(all_views_in_workspace);
    }
    views.extend(orphan_views);

    let favorites = self
      .section
      .section_op(txn, Section::Favorite, uid)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();
    let recent = self
      .section
      .section_op(txn, Section::Recent, uid)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let trash = self
      .section
      .section_op(txn, Section::Trash, uid)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let private = self
      .section
      .section_op(txn, Section::Private, uid)
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    Some(FolderData {
      uid,
      workspace,
      current_view,
      views,
      favorites,
      recent,
      trash,
      private,
    })
  }

  pub fn get_workspace_id<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)
  }

  pub fn move_view(
    &self,
    txn: &mut TransactionMut,
    view_id: &str,
    from: u32,
    to: u32,
    uid: i64,
  ) -> Option<Arc<View>> {
    let view = self.views.get_view_with_txn(txn, view_id, uid)?;
    self.views.move_child(txn, &view.parent_view_id, from, to);
    Some(view)
  }

  pub fn move_nested_view(
    &self,
    txn: &mut TransactionMut,
    view_id: &str,
    new_parent_id: &str,
    prev_view_id: Option<String>,
    uid: i64,
  ) -> Option<Arc<View>> {
    tracing::debug!("Move nested view: {}", view_id);
    let view = self.views.get_view_with_txn(txn, view_id, uid)?;
    let current_workspace_id = self.get_workspace_id_with_txn(txn)?;
    let parent_id = view.parent_view_id.as_str();

    let new_parent_view = self.views.get_view_with_txn(txn, new_parent_id, uid);

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
      .update_view_with_txn(UserId::from(uid), txn, view_id, |update| {
        update.set_bid(new_parent_id).done()
      });
    Some(view)
  }

  pub fn get_child_of_first_public_view<T: ReadTxn>(&self, txn: &T, uid: i64) -> Option<String> {
    self
      .get_workspace_id(txn)
      .and_then(|workspace_id| self.views.get_view(txn, &workspace_id, uid))
      .and_then(|root_view| {
        let first_public_space_view_id_with_child = root_view.children.iter().find(|space_id| {
          match self.views.get_view(txn, space_id, uid) {
            Some(space_view) => {
              let is_public_space = space_view
                .space_info()
                .map(|info| info.space_permission == SpacePermission::PublicToAll)
                .unwrap_or(false);
              let has_children = !space_view.children.is_empty();
              is_public_space && has_children
            },
            None => false,
          }
        });
        first_public_space_view_id_with_child.map(|v| v.id.clone())
      })
      .and_then(|first_public_space_view_id_with_child| {
        self
          .views
          .get_view(txn, &first_public_space_view_id_with_child, uid)
      })
      .and_then(|first_public_space_view_with_child| {
        first_public_space_view_with_child
          .children
          .iter()
          .next()
          .map(|first_child| first_child.id.clone())
      })
  }

  pub fn get_current_view<T: ReadTxn>(&self, txn: &T, uid: i64) -> Option<String> {
    // Fallback to CURRENT_VIEW if CURRENT_VIEW_FOR_USER is not present. This could happen for
    // workspace folder created by older version of the app before CURRENT_VIEW_FOR_USER is introduced.
    // If user cannot be found in CURRENT_VIEW_FOR_USER, use the first child of the first public space
    // which has children.
    let current_view_for_user_map = match self.meta.get(txn, CURRENT_VIEW_FOR_USER) {
      Some(YrsValue::YMap(map)) => Some(map),
      _ => None,
    };
    match current_view_for_user_map {
      Some(current_view_for_user) => {
        let view_for_user: Option<String> =
          current_view_for_user.get_with_txn(txn, uid.to_string().as_ref());
        view_for_user.or(self.get_child_of_first_public_view(txn, uid))
      },
      None => self.meta.get_with_txn(txn, CURRENT_VIEW),
    }
  }

  pub fn set_current_view(&self, txn: &mut TransactionMut, view: String, uid: i64) {
    let current_view_for_user = self.meta.get_or_init_map(txn, CURRENT_VIEW_FOR_USER);
    current_view_for_user.try_update(txn, uid.to_string(), view);
  }
}

pub fn default_folder_data(uid: i64, workspace_id: &str) -> FolderData {
  let workspace = Workspace {
    id: workspace_id.to_string(),
    name: "".to_string(),
    child_views: Default::default(),
    created_at: 0,
    created_by: None,
    last_edited_time: 0,
    last_edited_by: None,
  };
  FolderData {
    uid,
    workspace,
    current_view: "".to_string(),
    views: vec![],
    favorites: HashMap::new(),
    recent: HashMap::new(),
    trash: HashMap::new(),
    private: HashMap::new(),
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use crate::{
    Folder, FolderData, RepeatedViewIdentifier, SectionItem, SpaceInfo, UserId, View,
    ViewIdentifier, Workspace,
  };
  use collab::core::collab::default_client_id;
  use collab::{core::collab::CollabOptions, core::origin::CollabOrigin, preclude::Collab};

  #[test]
  pub fn test_set_and_get_current_view() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = "1234";
    let uid = 1;
    let options = CollabOptions::new(workspace_id.to_string(), default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let view_1 = View::new(
      "view_1".to_string(),
      workspace_id.to_string(),
      "View 1".to_string(),
      crate::ViewLayout::Document,
      Some(uid),
    );
    let view_1_id = view_1.id.clone();
    let view_2 = View::new(
      "view_2".to_string(),
      workspace_id.to_string(),
      "View 2".to_string(),
      crate::ViewLayout::Document,
      Some(uid),
    );
    let view_2_id = view_2.id.clone();
    let space_view = View {
      id: "space_1_id".to_string(),
      parent_view_id: workspace_id.to_string(),
      name: "Space 1".to_string(),
      children: RepeatedViewIdentifier::new(vec![
        ViewIdentifier::new(view_1_id.clone()),
        ViewIdentifier::new(view_2_id.clone()),
      ]),
      created_at: current_time,
      is_favorite: false,
      layout: crate::ViewLayout::Document,
      icon: None,
      created_by: None,
      last_edited_time: current_time,
      last_edited_by: None,
      is_locked: None,
      extra: Some(serde_json::to_string(&SpaceInfo::default()).unwrap()),
    };
    let space_view_id = space_view.id.clone();
    let workspace = Workspace {
      id: workspace_id.to_string(),
      name: "Workspace".to_string(),
      child_views: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(space_view_id.clone())]),
      created_at: current_time,
      created_by: Some(uid),
      last_edited_time: current_time,
      last_edited_by: Some(uid),
    };
    let folder_data = FolderData {
      uid,
      workspace,
      current_view: view_2.id.clone(),
      views: vec![space_view, view_1, view_2],
      favorites: Default::default(),
      recent: Default::default(),
      trash: Default::default(),
      private: Default::default(),
    };
    let mut folder = Folder::create(collab, None, folder_data);

    folder.set_current_view(view_2_id.clone(), uid);
    assert_eq!(folder.get_current_view(uid), Some(view_2_id.to_string()));
    // First visit from user 2, should return the first child of the first public space with children.
    assert_eq!(folder.get_current_view(2), Some(view_1_id.to_string()));
    folder.set_current_view(view_1_id.to_string(), 2);
    assert_eq!(folder.get_current_view(1), Some(view_2_id.to_string()));
    assert_eq!(folder.get_current_view(2), Some(view_1_id.to_string()));
  }

  #[test]
  pub fn test_move_section() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = "1234";
    let uid = 1;
    let options = CollabOptions::new(workspace_id.to_string(), default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let space_view_id = "space_view_id".to_string();
    let views: Vec<View> = (0..3)
      .map(|i| {
        View::new(
          format!("view_{:?}", i),
          space_view_id.clone(),
          format!("View {:?}", i),
          crate::ViewLayout::Document,
          Some(uid),
        )
      })
      .collect();
    let space_view = View {
      id: "space_1_id".to_string(),
      parent_view_id: workspace_id.to_string(),
      name: "Space 1".to_string(),
      children: RepeatedViewIdentifier::new(
        views
          .iter()
          .map(|view| ViewIdentifier::new(view.id.clone()))
          .collect(),
      ),
      created_at: current_time,
      is_favorite: false,
      layout: crate::ViewLayout::Document,
      icon: None,
      created_by: None,
      last_edited_time: current_time,
      last_edited_by: None,
      is_locked: None,
      extra: Some(serde_json::to_string(&SpaceInfo::default()).unwrap()),
    };
    let workspace = Workspace {
      id: workspace_id.to_string(),
      name: "Workspace".to_string(),
      child_views: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(space_view_id.clone())]),
      created_at: current_time,
      created_by: Some(uid),
      last_edited_time: current_time,
      last_edited_by: Some(uid),
    };
    let all_views: Vec<View> = views
      .iter()
      .chain(std::iter::once(&space_view))
      .cloned()
      .collect();
    let folder_data = FolderData {
      uid,
      workspace,
      current_view: Default::default(),
      views: all_views,
      favorites: HashMap::from([(
        UserId::from(uid),
        views
          .iter()
          .map(|view| SectionItem::new(view.id.clone()))
          .collect(),
      )]),
      recent: Default::default(),
      trash: Default::default(),
      private: Default::default(),
    };
    let mut folder = Folder::create(collab, None, folder_data);
    let favorite_sections = folder.get_all_favorites_sections(uid);
    let expected_favorites = vec![
      SectionItem::new("view_0".to_string()),
      SectionItem::new("view_1".to_string()),
      SectionItem::new("view_2".to_string()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
    folder.move_favorite_view_id("view_0", Some("view_1"), uid);
    let favorite_sections = folder.get_all_favorites_sections(uid);
    let expected_favorites = vec![
      SectionItem::new("view_1".to_string()),
      SectionItem::new("view_0".to_string()),
      SectionItem::new("view_2".to_string()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
    folder.move_favorite_view_id("view_2", None, uid);
    let favorite_sections = folder.get_all_favorites_sections(uid);
    let expected_favorites = vec![
      SectionItem::new("view_2".to_string()),
      SectionItem::new("view_1".to_string()),
      SectionItem::new("view_0".to_string()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
  }
}
