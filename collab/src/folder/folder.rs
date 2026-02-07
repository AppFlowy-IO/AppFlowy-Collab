use crate::core::collab::{CollabOptions, DataSource};
pub use crate::core::origin::CollabOrigin;
use crate::entity::CollabType;
use crate::entity::EncodedCollab;
use crate::entity::define::{FOLDER, FOLDER_META, FOLDER_WORKSPACE_ID};
use crate::preclude::*;
use crate::util::any_to_json_value;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use super::folder_observe::ViewChangeSender;
use super::hierarchy_builder::{FlattedViews, ParentChildViews};
use super::section::{Section, SectionItem, SectionMap};
use super::{
  FolderData, ParentChildRelations, SectionChangeSender, SpacePermission, TrashInfo, View,
  ViewChangeReceiver, ViewId, ViewUpdate, ViewsMap, Workspace,
};
use crate::entity::uuid_validation::WorkspaceId;
use crate::error::CollabError;

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
  pub fn open(mut collab: Collab, notifier: Option<FolderNotify>) -> Result<Self, CollabError> {
    let body = FolderBody::open(&mut collab, notifier)?;
    let folder = Folder { collab, body };
    if folder.get_workspace_id().is_none() {
      // When the folder is opened, the workspace id must be present.
      Err(CollabError::FolderMissingRequiredData(
        "missing workspace id".into(),
      ))
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
  ) -> Result<Self, CollabError> {
    let workspace_uuid = Uuid::parse_str(workspace_id)
      .map_err(|_| CollabError::Internal(anyhow!("Invalid workspace id format")))?;
    let options = CollabOptions::new(workspace_uuid, client_id).with_data_source(collab_doc_state);
    let collab = Collab::new_with_options(origin, options)?;
    Self::open(collab, None)
  }

  pub fn close(&self) {
    self.collab.remove_all_plugins();
  }

  pub fn validate(&self) -> Result<(), CollabError> {
    CollabType::Folder
      .validate_require_data(&self.collab)
      .map_err(|err| CollabError::FolderMissingRequiredData(err.to_string()))?;
    Ok(())
  }

  /// Returns the doc state and the state vector.
  pub fn encode_collab(&self) -> Result<EncodedCollab, CollabError> {
    self.collab.encode_collab_v1(|collab| {
      CollabType::Folder
        .validate_require_data(collab)
        .map_err(|err| CollabError::FolderMissingRequiredData(err.to_string()))
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
  ///   When uid is provided, includes user-specific sections. When uid is None, returns empty user-specific sections.
  ///
  /// * `None`: If the operation is unsuccessful (though it should typically not be the case as `Some`
  ///   is returned explicitly), it returns `None`.
  pub fn get_folder_data(&self, workspace_id: &str, uid: Option<i64>) -> Option<FolderData> {
    let txn = self.collab.transact();
    self.body.get_folder_data(&txn, workspace_id, uid)
  }

  /// Fetches the current workspace. The uid parameter is accepted for API consistency
  /// but not used as workspace data is shared across all users.
  ///
  /// This function fetches the ID of the current workspace from the meta object,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_workspace_info(
    &self,
    workspace_id: &WorkspaceId,
    uid: Option<i64>,
  ) -> Option<Workspace> {
    let txn = self.collab.transact();
    self.body.get_workspace_info(&txn, workspace_id, uid)
  }

  pub fn get_workspace_id(&self) -> Option<ViewId> {
    let txn = self.collab.transact();
    self.body.get_workspace_id(&txn)?.parse().ok()
  }

  /// Get all views. When uid is provided, includes user-specific data like is_favorite.
  /// When uid is None, returns base view data without user-specific enrichment.
  pub fn get_all_views(&self, uid: Option<i64>) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_all_views(&txn, uid)
  }

  /// Get multiple views by ids. When uid is provided, includes user-specific data like is_favorite.
  /// When uid is None, returns base view data without user-specific enrichment.
  pub fn get_views(&self, view_ids: &[ViewId], uid: Option<i64>) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_views(&txn, view_ids, uid)
  }

  /// Get all views belonging to a parent. When uid is provided, includes user-specific data.
  /// When uid is None, returns base view data without user-specific enrichment.
  pub fn get_views_belong_to(&self, parent_id: &ViewId, uid: Option<i64>) -> Vec<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_views_belong_to(&txn, parent_id, uid)
  }

  pub fn move_view(&mut self, view_id: &ViewId, from: u32, to: u32, uid: i64) -> Option<Arc<View>> {
    let mut txn = self.collab.transact_mut();
    self.body.move_view(&mut txn, view_id, from, to, Some(uid))
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
    view_id: &ViewId,
    new_parent_id: &ViewId,
    prev_view_id: Option<ViewId>,
    uid: i64,
  ) -> Option<Arc<View>> {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .move_nested_view(&mut txn, view_id, new_parent_id, prev_view_id, Some(uid))
  }

  pub fn set_current_view(&mut self, view_id: ViewId, uid: i64) {
    let mut txn = self.collab.transact_mut();
    self.body.set_current_view(&mut txn, view_id, Some(uid));
  }

  pub fn get_current_view(&self, uid: i64) -> Option<ViewId> {
    let txn = self.collab.transact();
    self.body.get_current_view(&txn, Some(uid))
  }

  pub fn update_view<F>(&mut self, view_id: &ViewId, f: F, uid: i64) -> Option<Arc<View>>
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    let mut txn = self.collab.transact_mut();
    self.body.views.update_view(&mut txn, view_id, f, uid)
  }

  pub fn delete_views(&mut self, views: Vec<ViewId>) {
    let mut txn = self.collab.transact_mut();
    self.body.views.delete_views(&mut txn, views);
  }

  // Section operations
  // Favorites
  pub fn add_favorite_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_favorite(true).done(),
          uid,
        );
      }
    }
  }

  pub fn delete_favorite_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_favorite(false).done(),
          uid,
        );
      }
    }
  }

  /// Retrieves the favorite views for a specific user.
  ///
  /// # How Sections Work
  ///
  /// Sections are **user-specific collections** stored in the collaborative folder.
  /// The folder maintains four predefined sections: Favorite, Recent, Trash, and Private.
  ///
  /// **Data Structure:**
  /// ```text
  /// Folder (CRDT)
  ///   └─ SectionMap
  ///       └─ "favorite" (Section)
  ///           ├─ "1" (uid) → [SectionItem { id: view_uuid, timestamp }, ...]
  ///           ├─ "2" (uid) → [SectionItem { id: view_uuid, timestamp }, ...]
  ///           └─ "3" (uid) → [SectionItem { id: view_uuid, timestamp }, ...]
  /// ```
  ///
  /// Each section type (favorite/recent/trash/private) contains a map where:
  /// - **Key**: User ID (as string representation of i64)
  /// - **Value**: Array of `SectionItem` structs, each containing:
  ///   - `id`: ViewId (UUID) of the view in this section
  ///   - `timestamp`: When the view was added to this section
  ///
  /// This architecture allows multiple users to collaborate on the same folder
  /// while maintaining separate personal collections (favorites, recent views, etc.).
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID to query favorites for
  ///   - `Some(uid)`: Returns the favorite views for the specified user
  ///   - `None`: Returns an empty vector (no user context = no user-specific data)
  ///
  /// # Returns
  ///
  /// A vector of `SectionItem` structs representing the user's favorite views.
  /// Each item contains the view's UUID and the timestamp when it was favorited.
  /// Returns empty vector if:
  /// - `uid` is `None`
  /// - The user has no favorites
  /// - The favorite section doesn't exist
  ///
  /// # Why `Option<i64>` for Query Operations?
  ///
  /// This method uses `Option<i64>` (not required `i64`) because:
  /// 1. **Safe degradation**: Can return meaningful result (empty) when uid is unknown
  /// 2. **Flexible usage**: Callers can query without having user context
  /// 3. **No side effects**: Read-only operation that doesn't modify data
  ///
  /// Compare with mutation operations like `add_favorite_view_ids(uid: i64)` which
  /// require `i64` because adding favorites without a user ID would be meaningless.
  ///
  /// # Related Methods
  ///
  /// - [`get_all_favorites_sections`]: Gets favorites across all users (admin mode)
  /// - [`add_favorite_view_ids`]: Adds views to user's favorites (requires `i64`)
  /// - [`delete_favorite_view_ids`]: Removes views from user's favorites (requires `i64`)
  /// - [`get_my_trash_sections`]: Similar pattern for trash section
  /// - [`get_my_private_sections`]: Similar pattern for private section
  /// - [`get_my_recent_sections`]: Similar pattern for recent section
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// # use collab::folder::{Folder, UserId};
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 123_i64;
  /// # let target_view_id = uuid::Uuid::nil();
  /// // Get favorites for a specific user
  /// let user_id = UserId::from(123);
  /// let favorites = folder.get_my_favorite_sections(Some(user_id.as_i64()));
  /// for item in favorites {
  ///     println!("View {} was favorited at {}", item.id, item.timestamp);
  /// }
  ///
  /// // Query without user context (returns empty)
  /// let no_favorites = folder.get_my_favorite_sections(None);
  /// assert!(no_favorites.is_empty());
  ///
  /// // Check if a view is favorited
  /// let is_favorited = folder.get_my_favorite_sections(Some(uid))
  ///     .iter()
  ///     .any(|item| item.id == target_view_id);
  /// ```
  ///
  /// # Implementation Details
  ///
  /// Internally, this method:
  /// 1. Creates a read transaction on the Collab CRDT
  /// 2. Gets a `SectionOperation` for the Favorite section with the given uid
  /// 3. Calls `get_all_section_item()` which:
  ///    - Looks up the array at key `uid.to_string()` in the favorite section
  ///    - Deserializes each Yrs array element into a `SectionItem`
  ///    - Returns the vector of items
  ///
  /// The operation is **read-only** and **lock-free** thanks to CRDT properties.
  pub fn get_my_favorite_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let Some(uid) = uid else {
      return vec![];
    };
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Favorite, Some(uid))
      .map(|op| op.get_all_section_item(&txn))
      .unwrap_or_default()
  }

  /// Retrieves favorite views across all users, with optional filtering by user.
  ///
  /// This is the "admin mode" variant of [`get_my_favorite_sections`]. While `get_my_*`
  /// returns empty when uid is None, this method returns **all users' favorites** when
  /// uid is None.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID for filtering
  ///   - `Some(uid)`: Returns only the favorites for the specified user (same as `get_my_favorite_sections`)
  ///   - `None`: Returns favorites from **all users** (admin/global query mode)
  ///
  /// # Returns
  ///
  /// A flattened vector of all `SectionItem` entries across the queried user(s).
  ///
  /// # Behavior When uid is None
  ///
  /// **This is the key difference from `get_my_favorite_sections`:**
  ///
  /// When `uid` is `None`, this method returns favorites from **all users**, not an empty vector.
  /// This enables admin/debugging operations like:
  /// - Viewing all favorited content across the workspace
  /// - Finding popular/frequently favorited views
  /// - Debugging favorite state
  ///
  /// ```text
  /// get_my_favorite_sections(None)   → []  (empty - no user context)
  /// get_all_favorites_sections(None) → [user1's favorites, user2's favorites, ...] (all users)
  /// ```
  ///
  /// # Use Cases
  ///
  /// ## Admin Dashboard - Popular Views
  /// ```rust,no_run
  /// # use collab::folder::{Folder, ViewId};
  /// # use std::collections::HashMap;
  /// # let folder: Folder = unimplemented!();
  /// // Get all favorited views across all users
  /// let all_favorites = folder.get_all_favorites_sections(None);
  /// let view_counts: HashMap<ViewId, usize> = all_favorites
  ///     .iter()
  ///     .fold(HashMap::new(), |mut map, item| {
  ///         *map.entry(item.id).or_insert(0) += 1;
  ///         map
  ///     });
  ///
  /// // Find most favorited views
  /// let popular = view_counts
  ///     .into_iter()
  ///     .filter(|(_, count)| *count >= 3)
  ///     .collect::<Vec<_>>();
  /// ```
  ///
  /// ## Check Specific User (Alternative to get_my_*)
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// // These are equivalent:
  /// let favorites_a = folder.get_my_favorite_sections(Some(uid));
  /// let favorites_b = folder.get_all_favorites_sections(Some(uid));
  /// assert_eq!(favorites_a, favorites_b);
  /// ```
  ///
  /// # Related Methods
  ///
  /// - [`get_my_favorite_sections`]: User-scoped version (returns empty when uid is None)
  /// - [`add_favorite_view_ids`]: Add favorites for a user (requires `i64`)
  /// - [`delete_favorite_view_ids`]: Remove favorites for a user (requires `i64`)
  pub fn get_all_favorites_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Favorite, uid)
      .map(|op| op.get_sections(&txn))
      .unwrap_or_default()
      .into_iter()
      .flat_map(|(_user_id, items)| items)
      .collect()
  }

  pub fn remove_all_my_favorite_sections(&mut self, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Favorite, Some(uid))
    {
      op.clear(&mut txn);
    }
  }

  pub fn move_favorite_view_id(&mut self, id: &str, prev_id: Option<&str>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Favorite, Some(uid))
    {
      op.move_section_item_with_txn(&mut txn, id, prev_id);
    }
  }

  // Trash
  pub fn add_trash_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_trash(true).done(),
          uid,
        );
      }
    }
  }

  pub fn delete_trash_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_trash(false).done(),
          uid,
        );
      }
    }
  }

  /// Retrieves the trashed views for a specific user.
  ///
  /// The trash section contains views that have been deleted by the user but not yet
  /// permanently removed. This allows for a "trash bin" functionality where users can
  /// recover accidentally deleted views.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID to query trash for
  ///   - `Some(uid)`: Returns the trashed views for the specified user
  ///   - `None`: Returns an empty vector (no user = no trash to show)
  ///
  /// # Returns
  ///
  /// A vector of `SectionItem` structs where:
  /// - `item.id`: The view UUID that was trashed
  /// - `item.timestamp`: When the view was moved to trash (for auto-deletion policies)
  ///
  /// Returns empty vector if:
  /// - `uid` is `None` (no user context)
  /// - The user has no items in trash
  /// - The trash section doesn't exist
  ///
  /// # Behavior When uid is None
  ///
  /// When `uid` is `None`, this method returns an empty vector because trash is
  /// **user-specific** data. Without knowing which user's trash to query, the method
  /// cannot return meaningful results. This design:
  /// - Prevents accidentally showing another user's deleted items
  /// - Fails safely (empty instead of error)
  /// - Maintains consistency with other user-scoped queries
  ///
  /// If you need to query trash across all users (e.g., for admin purposes), use
  /// [`get_all_trash_sections`] instead.
  ///
  /// # Common Use Cases
  ///
  /// ## Display Trash Bin
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let current_user_id = 1_i64;
  /// let trash_items = folder.get_my_trash_sections(Some(current_user_id));
  /// for item in trash_items {
  ///     let _ = (item.id, item.timestamp);
  /// }
  /// ```
  ///
  /// ## Auto-Delete Old Items
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # fn current_timestamp() -> i64 {
  /// #     std::time::SystemTime::now()
  /// #         .duration_since(std::time::UNIX_EPOCH)
  /// #         .unwrap()
  /// #         .as_millis() as i64
  /// # }
  /// # let mut folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// let trash = folder.get_my_trash_sections(Some(uid));
  /// let thirty_days_ago = current_timestamp() - (30 * 24 * 60 * 60 * 1000);
  ///
  /// let to_permanently_delete: Vec<_> = trash
  ///     .iter()
  ///     .filter(|item| item.timestamp < thirty_days_ago)
  ///     .map(|item| item.id.to_string())
  ///     .collect();
  ///
  /// if !to_permanently_delete.is_empty() {
  ///     folder.delete_trash_view_ids(to_permanently_delete, uid);
  /// }
  /// ```
  ///
  /// ## Check if View is in Trash
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// # let target_view_id = uuid::Uuid::nil();
  /// let is_trashed = folder.get_my_trash_sections(Some(uid))
  ///     .iter()
  ///     .any(|item| item.id == target_view_id);
  /// ```
  ///
  /// # Related Methods
  ///
  /// - [`add_trash_view_ids`]: Move views to trash (requires `i64`)
  /// - [`delete_trash_view_ids`]: Permanently delete from trash (requires `i64`)
  /// - [`get_all_trash_sections`]: Query trash across all users (admin mode)
  /// - [`get_my_trash_info`]: Get trash with additional view metadata
  pub fn get_my_trash_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let Some(uid) = uid else {
      return vec![];
    };
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Trash, Some(uid))
      .map(|op| op.get_all_section_item(&txn))
      .unwrap_or_default()
  }

  /// Retrieves trashed views across all users, with optional filtering by user.
  ///
  /// This is the "admin mode" variant of [`get_my_trash_sections`]. While `get_my_trash_sections`
  /// returns empty when uid is None, this method returns **all users' trash** when uid is None.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID for filtering
  ///   - `Some(uid)`: Returns only the trash for the specified user
  ///   - `None`: Returns trash from **all users** (admin/cleanup mode)
  ///
  /// # Returns
  ///
  /// A flattened vector of all trashed `SectionItem` entries across the queried user(s).
  ///
  /// # Behavior When uid is None
  ///
  /// When `uid` is `None`, this method returns trash items from **all users**:
  ///
  /// ```text
  /// get_my_trash_sections(None)   → []  (empty - no user context)
  /// get_all_trash_sections(None) → [user1's trash, user2's trash, ...] (all users)
  /// ```
  ///
  /// This is useful for:
  /// - Admin cleanup operations (find all deleted content)
  /// - Global auto-deletion policies (remove items older than N days across all users)
  /// - Debugging trash state
  /// - Recovering content when user ID is unknown
  ///
  /// # Use Cases
  ///
  /// ## Global Cleanup - Delete Old Trash
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # fn current_timestamp() -> i64 {
  /// #     std::time::SystemTime::now()
  /// #         .duration_since(std::time::UNIX_EPOCH)
  /// #         .unwrap()
  /// #         .as_millis() as i64
  /// # }
  /// # let folder: Folder = unimplemented!();
  /// // Find all trash items older than 30 days across ALL users
  /// let all_trash = folder.get_all_trash_sections(None);
  /// let thirty_days_ago = current_timestamp() - (30 * 24 * 60 * 60 * 1000);
  ///
  /// let old_trash: Vec<_> = all_trash
  ///     .into_iter()
  ///     .filter(|item| item.timestamp < thirty_days_ago)
  ///     .collect();
  ///
  /// // Note: Actual deletion requires uid per item, so you'd need to
  /// // track which user owns which trash item separately
  /// ```
  ///
  /// ## Admin Dashboard - Trash Statistics
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// let all_trash = folder.get_all_trash_sections(None);
  /// println!("Total items in trash across all users: {}", all_trash.len());
  ///
  /// // Calculate trash size per user (requires additional tracking)
  /// ```
  ///
  /// # Related Methods
  ///
  /// - [`get_my_trash_sections`]: User-scoped version (returns empty when uid is None)
  /// - [`get_my_trash_info`]: User-scoped with view names
  /// - [`add_trash_view_ids`]: Move views to trash (requires `i64`)
  /// - [`delete_trash_view_ids`]: Permanently delete from trash (requires `i64`)
  pub fn get_all_trash_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Trash, uid)
      .map(|op| op.get_sections(&txn))
      .unwrap_or_default()
      .into_iter()
      .flat_map(|(_user_id, items)| items)
      .collect()
  }

  pub fn remove_all_my_trash_sections(&mut self, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Trash, Some(uid))
    {
      op.clear(&mut txn);
    }
  }

  pub fn move_trash_view_id(&mut self, id: &str, prev_id: Option<&str>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Trash, Some(uid))
    {
      op.move_section_item_with_txn(&mut txn, id, prev_id);
    }
  }

  // Private
  pub fn add_private_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_private(true).done(),
          uid,
        );
      }
    }
  }

  pub fn delete_private_view_ids(&mut self, ids: Vec<String>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    for id in ids {
      if let Ok(view_uuid) = uuid::Uuid::parse_str(&id) {
        self.body.views.update_view(
          &mut txn,
          &view_uuid,
          |update| update.set_private(false).done(),
          uid,
        );
      }
    }
  }

  /// Retrieves the private views for a specific user.
  ///
  /// The private section contains views that are marked as private/personal to the user
  /// and should be hidden from other collaborators. This enables personal workspace areas
  /// within a shared collaborative folder.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID to query private views for
  ///   - `Some(uid)`: Returns the private views for the specified user
  ///   - `None`: Returns an empty vector (no user = no private views to show)
  ///
  /// # Returns
  ///
  /// A vector of `SectionItem` structs where:
  /// - `item.id`: The view UUID marked as private
  /// - `item.timestamp`: When the view was marked as private
  ///
  /// Returns empty vector if:
  /// - `uid` is `None` (no user context)
  /// - The user has no private views
  /// - The private section doesn't exist
  ///
  /// # Behavior When uid is None
  ///
  /// When `uid` is `None`, this method returns an empty vector because private views are
  /// **inherently user-specific**. The concept of "private" only makes sense in the context
  /// of a specific user - views are private *to someone*. Without a user ID:
  /// - Cannot determine whose private views to return
  /// - Returning all users' private views would violate privacy
  /// - Empty result is the safest, most consistent behavior
  ///
  /// For admin operations that need to see all private views across users, use
  /// [`get_all_private_sections`] instead.
  ///
  /// # Privacy Semantics
  ///
  /// **Important**: This method only returns which views are *marked* as private. The actual
  /// visibility enforcement (hiding these views from other users) must be implemented by
  /// the application layer. The section system provides the data structure, not the access
  /// control mechanism.
  ///
  /// # Common Use Cases
  ///
  /// ## Filter Out Other Users' Private Views
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let current_user_id = 1_i64;
  /// let all_views = folder.get_all_views(Some(current_user_id));
  /// let other_private_views = folder.get_all_private_sections(Some(current_user_id));
  ///
  /// let visible_views: Vec<_> = all_views
  ///     .into_iter()
  ///     .filter(|view| {
  ///         !other_private_views.iter().any(|private| private.id == view.id)
  ///     })
  ///     .collect();
  /// ```
  ///
  /// ## Show User's Personal Workspace
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// let my_private = folder.get_my_private_sections(Some(uid));
  /// if !my_private.is_empty() {
  ///     let _ = &my_private;
  /// }
  /// ```
  ///
  /// ## Check if View is Private
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// # let view_id = uuid::Uuid::nil();
  /// let is_private = folder.get_my_private_sections(Some(uid))
  ///     .iter()
  ///     .any(|item| item.id == view_id);
  /// ```
  ///
  /// # Related Methods
  ///
  /// - [`add_private_view_ids`]: Mark views as private (requires `i64`)
  /// - [`delete_private_view_ids`]: Unmark views as private (requires `i64`)
  /// - [`get_all_private_sections`]: Query private views across all users (admin mode)
  pub fn get_my_private_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let Some(uid) = uid else {
      return vec![];
    };
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Private, Some(uid))
      .map(|op| op.get_all_section_item(&txn))
      .unwrap_or_default()
  }

  /// Retrieves private views across all users, with optional filtering by user.
  ///
  /// This is the "admin mode" variant of [`get_my_private_sections`]. While `get_my_private_sections`
  /// returns empty when uid is None, this method returns **all users' private views** when uid is None.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID for filtering
  ///   - `Some(uid)`: Returns only the private views for the specified user
  ///   - `None`: Returns private views from **all users** (admin/audit mode)
  ///
  /// # Returns
  ///
  /// A flattened vector of all private `SectionItem` entries across the queried user(s).
  ///
  /// # Behavior When uid is None
  ///
  /// When `uid` is `None`, this method returns private items from **all users**:
  ///
  /// ```text
  /// get_my_private_sections(None)   → []  (empty - no user context)
  /// get_all_private_sections(None) → [user1's private, user2's private, ...] (all users)
  /// ```
  ///
  /// **Privacy Note**: Returning all users' private views may have privacy implications.
  /// This method should typically only be called:
  /// - In admin/debugging contexts
  /// - For workspace-level operations (e.g., migration, backup)
  /// - When implementing view filtering logic (to hide *other* users' private views)
  ///
  /// # Use Cases
  ///
  /// ## Filter Out Other Users' Private Views
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let current_user_id = 1_i64;
  /// // Get all views, then filter out views private to other users
  /// let all_views = folder.get_all_views(Some(current_user_id));
  /// let my_private_views = folder.get_my_private_sections(Some(current_user_id));
  /// let others_private_views = folder.get_all_private_sections(None);
  ///
  /// let visible_to_me: Vec<_> = all_views
  ///     .into_iter()
  ///     .filter(|view| {
  ///         // Show if it's my private view OR not private to anyone else
  ///         my_private_views.iter().any(|p| p.id == view.id)
  ///             || !others_private_views.iter().any(|p| p.id == view.id)
  ///     })
  ///     .collect();
  /// ```
  ///
  /// ## Admin Audit - Find All Private Content
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// let all_private = folder.get_all_private_sections(None);
  /// println!("Total private views across workspace: {}", all_private.len());
  ///
  /// // Identify users with private content (requires user tracking)
  /// ```
  ///
  /// ## Migration/Backup Operations
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// // When migrating workspace, preserve all private view metadata
  /// let all_private = folder.get_all_private_sections(None);
  /// let _ = all_private;
  /// ```
  ///
  /// # Related Methods
  ///
  /// - [`get_my_private_sections`]: User-scoped version (returns empty when uid is None)
  /// - [`add_private_view_ids`]: Mark views as private (requires `i64`)
  /// - [`delete_private_view_ids`]: Unmark views as private (requires `i64`)
  pub fn get_all_private_sections(&self, uid: Option<i64>) -> Vec<SectionItem> {
    let txn = self.collab.transact();
    self
      .body
      .section
      .section_op(&txn, Section::Private, uid)
      .map(|op| op.get_sections(&txn))
      .unwrap_or_default()
      .into_iter()
      .flat_map(|(_user_id, items)| items)
      .collect()
  }

  pub fn remove_all_my_private_sections(&mut self, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Private, Some(uid))
    {
      op.clear(&mut txn);
    }
  }

  pub fn move_private_view_id(&mut self, id: &str, prev_id: Option<&str>, uid: i64) {
    let mut txn = self.collab.transact_mut();
    if let Some(op) = self
      .body
      .section
      .section_op(&txn, Section::Private, Some(uid))
    {
      op.move_section_item_with_txn(&mut txn, id, prev_id);
    }
  }

  /// Retrieves enriched trash information for a specific user.
  ///
  /// This is an enhanced version of [`get_my_trash_sections`] that includes additional
  /// view metadata (name) for each trashed item. This is useful for displaying trash bins
  /// in the UI where you need to show the view name, not just its ID.
  ///
  /// # Parameters
  ///
  /// * `uid` - Optional user ID to query trash info for
  ///   - `Some(uid)`: Returns trash info for the specified user
  ///   - `None`: Returns an empty vector (no user = no trash to show)
  ///
  /// # Returns
  ///
  /// A vector of `TrashInfo` structs where each contains:
  /// - `id`: ViewId (UUID) of the trashed view
  /// - `name`: Human-readable name of the view (e.g., "My Document")
  /// - `created_at`: Timestamp when the view was moved to trash
  ///
  /// Returns empty vector if:
  /// - `uid` is `None` (no user context)
  /// - The user has no items in trash
  /// - The trash section doesn't exist
  ///
  /// **Note**: If a view ID exists in the trash section but the view itself has been
  /// permanently deleted or its name cannot be retrieved, that item will be **omitted**
  /// from the results (via `flat_map` semantics). This ensures the returned data is
  /// always consistent and displayable.
  ///
  /// # Behavior When uid is None
  ///
  /// When `uid` is `None`, returns an empty vector because:
  /// - Trash is user-specific data
  /// - Cannot determine which user's trash to query
  /// - Prevents privacy leaks (showing other users' deleted items)
  /// - Consistent with [`get_my_trash_sections`] behavior
  ///
  /// For admin operations, use `get_my_trash_sections` with all user IDs manually,
  /// as there's no `get_all_trash_info` variant (by design, to prevent accidentally
  /// exposing sensitive deleted data).
  ///
  /// # Comparison with get_my_trash_sections
  ///
  /// | Method | Returns | View Name | Use When |
  /// |--------|---------|-----------|----------|
  /// | `get_my_trash_sections` | `Vec<SectionItem>` | No | Need just IDs/timestamps |
  /// | `get_my_trash_info` | `Vec<TrashInfo>` | Yes | Displaying UI |
  ///
  /// Use `get_my_trash_info` when you need to show trash items to the user with readable
  /// names. Use `get_my_trash_sections` when you only need view IDs (e.g., checking if
  /// a view is trashed).
  ///
  /// # Common Use Cases
  ///
  /// ## Display Trash Bin UI
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let folder: Folder = unimplemented!();
  /// # let current_user_id = 1_i64;
  /// let trash_info = folder.get_my_trash_info(Some(current_user_id));
  /// for item in trash_info {
  ///     let _ = (item.id, &item.name, item.created_at);
  /// }
  /// ```
  ///
  /// ## Restore Deleted View by Name
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # let mut folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// let trash = folder.get_my_trash_info(Some(uid));
  /// if let Some(item) = trash.iter().find(|t| t.name == "Important Doc") {
  ///     folder.delete_trash_view_ids(vec![item.id.to_string()], uid);
  ///     println!("Restored: {}", item.name);
  /// }
  /// ```
  ///
  /// ## Show Time Since Deletion
  /// ```rust,no_run
  /// # use collab::folder::Folder;
  /// # fn current_timestamp() -> i64 {
  /// #     std::time::SystemTime::now()
  /// #         .duration_since(std::time::UNIX_EPOCH)
  /// #         .unwrap()
  /// #         .as_millis() as i64
  /// # }
  /// # let folder: Folder = unimplemented!();
  /// # let uid = 1_i64;
  /// let trash = folder.get_my_trash_info(Some(uid));
  /// let now = current_timestamp();
  ///
  /// for item in trash {
  ///     let days_ago = (now - item.created_at) / (24 * 60 * 60 * 1000);
  ///     println!("{} (deleted {} days ago)", item.name, days_ago);
  /// }
  /// ```
  ///
  /// # Implementation Details
  ///
  /// Internally, this method:
  /// 1. Calls `get_my_trash_sections(uid)` to get the list of trashed view IDs
  /// 2. For each `SectionItem`, looks up the view name from the CRDT
  /// 3. Combines ID + name + timestamp into `TrashInfo`
  /// 4. Uses `flat_map` to filter out items where name lookup fails
  ///
  /// The operation requires two CRDT lookups per item (section + view name), so for
  /// very large trash bins, `get_my_trash_sections` may be more efficient if you only
  /// need IDs.
  ///
  /// # Related Methods
  ///
  /// - [`get_my_trash_sections`]: Get trash without view names (more efficient)
  /// - [`add_trash_view_ids`]: Move views to trash (requires `i64`)
  /// - [`delete_trash_view_ids`]: Permanently delete from trash (requires `i64`)
  pub fn get_my_trash_info(&self, uid: Option<i64>) -> Vec<TrashInfo> {
    let Some(uid_val) = uid else {
      return vec![];
    };
    let txn = self.collab.transact();
    self
      .get_my_trash_sections(Some(uid_val))
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

  /// Get a view by id. When uid is provided, includes user-specific data like is_favorite.
  /// When uid is None, returns base view data without user-specific enrichment.
  pub fn get_view(&self, view_id: &ViewId, uid: Option<i64>) -> Option<Arc<View>> {
    let txn = self.collab.transact();
    self.body.views.get_view(&txn, view_id, uid)
  }

  pub fn is_view_in_section(&self, section: Section, view_id: &ViewId, uid: Option<i64>) -> bool {
    let txn = self.collab.transact();
    if let Some(uid) = uid {
      if let Some(op) = self.body.section.section_op(&txn, section, Some(uid)) {
        op.contains_with_txn(&txn, view_id)
      } else {
        false
      }
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
  pub fn get_view_recursively(&self, view_id: &ViewId, uid: Option<i64>) -> Vec<View> {
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

pub fn check_folder_is_valid(collab: &Collab) -> Result<String, CollabError> {
  let txn = collab.transact();
  let meta: MapRef = collab
    .data
    .get_with_path(&txn, vec![FOLDER, FOLDER_META])
    .ok_or_else(|| CollabError::FolderMissingRequiredData("No meta data".to_string()))?;
  match meta.get_with_txn::<_, String>(&txn, FOLDER_WORKSPACE_ID) {
    None => Err(CollabError::FolderMissingRequiredData(
      "No workspace id".to_string(),
    )),
    Some(workspace_id) => {
      if workspace_id.is_empty() {
        Err(CollabError::FolderMissingRequiredData(
          "No workspace id".to_string(),
        ))
      } else {
        Ok(workspace_id)
      }
    },
  }
}

pub struct FolderBody {
  pub root: MapRef,
  pub views: Arc<ViewsMap>,
  pub section: Arc<SectionMap>,
  pub meta: MapRef,
  #[allow(dead_code)]
  notifier: Option<FolderNotify>,
}

impl FolderBody {
  pub fn open(collab: &mut Collab, notifier: Option<FolderNotify>) -> Result<Self, CollabError> {
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
    let root = collab.data.get_or_init_map(&mut txn, FOLDER);

    // create the folder data
    let views: MapRef = root.get_or_init(&mut txn, VIEWS);
    let section: MapRef = root.get_or_init(&mut txn, SECTION);
    let meta: MapRef = root.get_or_init(&mut txn, FOLDER_META);
    let parent_child_relations = Arc::new(ParentChildRelations::new(
      root.get_or_init(&mut txn, PARENT_CHILD_VIEW_RELATION),
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
      let workspace_id = folder_data.workspace.id;
      views.insert(
        &mut txn,
        folder_data.workspace.into(),
        None,
        folder_data.uid,
      );

      for view in folder_data.views {
        views.insert(&mut txn, view, None, folder_data.uid);
      }

      meta.insert(&mut txn, FOLDER_WORKSPACE_ID, workspace_id.to_string());
      // For compatibility with older collab library which doesn't use CURRENT_VIEW_FOR_USER.
      if let Some(current_view) = folder_data.current_view {
        meta.insert(&mut txn, CURRENT_VIEW, current_view.to_string());
        let current_view_for_user = meta.get_or_init_map(&mut txn, CURRENT_VIEW_FOR_USER);
        current_view_for_user.insert(
          &mut txn,
          folder_data.uid.to_string(),
          current_view.to_string(),
        );
      }

      if let Some(fav_section) = section.section_op(&txn, Section::Favorite, Some(folder_data.uid))
      {
        for (uid, sections) in folder_data.favorites {
          fav_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
        }
      }

      if let Some(trash_section) = section.section_op(&txn, Section::Trash, Some(folder_data.uid)) {
        for (uid, sections) in folder_data.trash {
          trash_section.add_sections_for_user_with_txn(&mut txn, &uid, sections);
        }
      }
    }
    Self {
      root,
      views,
      section,
      meta,
      notifier,
    }
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
    view_id: &ViewId,
    visited: &mut HashSet<String>,
    accumulated_views: &mut Vec<View>,
    uid: Option<i64>,
  ) {
    let mut stack = vec![*view_id];
    while let Some(current_id) = stack.pop() {
      if !visited.insert(current_id.to_string()) {
        continue;
      }
      if let Some(parent_view) = self.views.get_view_with_txn(txn, &current_id, uid) {
        accumulated_views.push(parent_view.as_ref().clone());
        for child in parent_view.children.items.iter().rev() {
          stack.push(child.id);
        }
      }
    }
  }

  pub fn get_workspace_info<T: ReadTxn>(
    &self,
    txn: &T,
    workspace_id: &WorkspaceId,
    uid: Option<i64>,
  ) -> Option<Workspace> {
    let folder_workspace_id: String = self.meta.get_with_txn(txn, FOLDER_WORKSPACE_ID)?;
    // Convert workspace_id UUID to string for comparison
    let uuid_workspace_id = workspace_id.to_string();
    if folder_workspace_id != uuid_workspace_id {
      error!("Workspace id not match when get current workspace");
      return None;
    }

    let view = self.views.get_view_with_txn(txn, workspace_id, uid)?;
    Some(Workspace::from(view.as_ref()))
  }

  pub fn get_folder_data<T: ReadTxn>(
    &self,
    txn: &T,
    workspace_id: &str,
    uid: Option<i64>,
  ) -> Option<FolderData> {
    let uid = uid?;
    let folder_workspace_id = self.get_workspace_id_with_txn(txn)?;
    // Parse workspace_id as UUID, return None if invalid
    let workspace_uuid = match uuid::Uuid::parse_str(workspace_id) {
      Ok(id) => id,
      Err(_) => {
        error!("Invalid workspace id format: {}", workspace_id);
        return None;
      },
    };
    let uuid_workspace_id = workspace_uuid.to_string();
    if folder_workspace_id != uuid_workspace_id {
      error!(
        "Workspace id not match when get folder data, expected: {}, actual: {}",
        workspace_id, folder_workspace_id
      );
      return None;
    }
    let workspace = Workspace::from(
      self
        .views
        .get_view_with_txn(txn, &workspace_uuid, Some(uid))?
        .as_ref(),
    );
    let current_view = self.get_current_view(txn, Some(uid));
    let mut views = vec![];
    let orphan_views = self
      .views
      .get_orphan_views_with_txn(txn, Some(uid))
      .iter()
      .map(|view| view.as_ref().clone())
      .collect::<Vec<View>>();
    for view in self
      .views
      .get_views_belong_to(txn, &workspace_uuid, Some(uid))
    {
      let mut all_views_in_workspace = vec![];
      self.get_view_recursively_with_txn(
        txn,
        &view.id,
        &mut HashSet::default(),
        &mut all_views_in_workspace,
        Some(uid),
      );
      views.extend(all_views_in_workspace);
    }
    views.extend(orphan_views);

    let favorites = self
      .section
      .section_op(txn, Section::Favorite, Some(uid))
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();
    let recent = self
      .section
      .section_op(txn, Section::Recent, Some(uid))
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let trash = self
      .section
      .section_op(txn, Section::Trash, Some(uid))
      .map(|op| op.get_sections(txn))
      .unwrap_or_default();

    let private = self
      .section
      .section_op(txn, Section::Private, Some(uid))
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

  pub async fn observe_view_changes(&self, uid: Option<i64>) {
    self.views.observe_view_change(uid, HashMap::new()).await;
  }

  pub async fn subscribe_view_changes(&self) -> Option<ViewChangeReceiver> {
    self
      .notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.subscribe())
  }

  pub fn move_view(
    &self,
    txn: &mut TransactionMut,
    view_id: &ViewId,
    from: u32,
    to: u32,
    uid: Option<i64>,
  ) -> Option<Arc<View>> {
    let view = self.views.get_view_with_txn(txn, view_id, uid)?;
    if let Some(parent_uuid) = &view.parent_view_id {
      self.views.move_child(txn, parent_uuid, from, to);
    }
    Some(view)
  }

  pub fn move_nested_view(
    &self,
    txn: &mut TransactionMut,
    view_id: &ViewId,
    new_parent_id: &ViewId,
    prev_view_id: Option<ViewId>,
    uid: Option<i64>,
  ) -> Option<Arc<View>> {
    let uid = uid?;
    tracing::debug!("Move nested view: {}", view_id);
    let view = self.views.get_view_with_txn(txn, view_id, Some(uid))?;
    let current_workspace_id = self.get_workspace_id_with_txn(txn)?;
    let parent_id = &view.parent_view_id;

    let new_parent_view = self.views.get_view_with_txn(txn, new_parent_id, Some(uid));

    // If the new parent is not a view, it must be a workspace.
    // Check if the new parent is the current workspace, as moving out of the current workspace is not supported yet.
    let current_workspace_uuid = uuid::Uuid::parse_str(&current_workspace_id).ok();
    if Some(*new_parent_id) != current_workspace_uuid && new_parent_view.is_none() {
      tracing::warn!("Unsupported move out current workspace: {}", view_id);
      return None;
    }

    // dissociate the child from its parent
    if let Some(parent_uuid) = parent_id {
      self
        .views
        .dissociate_parent_child_with_txn(txn, parent_uuid, view_id);
    }
    // associate the child with its new parent and place it after the prev_view_id. If the prev_view_id is None,
    // place it as the first child.
    self
      .views
      .associate_parent_child_with_txn(txn, new_parent_id, view_id, prev_view_id);
    // Update the view's parent ID.
    self
      .views
      .update_view_with_txn(UserId::from(uid), txn, view_id, |update| {
        update.set_bid(new_parent_id.to_string()).done()
      });
    Some(view)
  }

  pub fn get_child_of_first_public_view<T: ReadTxn>(
    &self,
    txn: &T,
    uid: Option<i64>,
  ) -> Option<ViewId> {
    self
      .get_workspace_id(txn)
      .and_then(|workspace_id| uuid::Uuid::parse_str(&workspace_id).ok())
      .and_then(|uuid| self.views.get_view_with_txn(txn, &uuid, uid))
      .and_then(|root_view| {
        let first_public_space_view_id_with_child = root_view.children.iter().find(|space_id| {
          match self.views.get_view_with_txn(txn, &space_id.id, uid) {
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
        first_public_space_view_id_with_child.map(|v| v.id)
      })
      .and_then(|first_public_space_view_id_with_child| {
        self
          .views
          .get_view_with_txn(txn, &first_public_space_view_id_with_child, uid)
      })
      .and_then(|first_public_space_view_with_child| {
        first_public_space_view_with_child
          .children
          .iter()
          .next()
          .map(|first_child| first_child.id)
      })
  }

  pub fn get_current_view<T: ReadTxn>(&self, txn: &T, uid: Option<i64>) -> Option<ViewId> {
    // Fallback to CURRENT_VIEW if CURRENT_VIEW_FOR_USER is not present. This could happen for
    // workspace folder created by older version of the app before CURRENT_VIEW_FOR_USER is introduced.
    // If user cannot be found in CURRENT_VIEW_FOR_USER, use the first child of the first public space
    // which has children.
    let current_view_for_user_map = match self.meta.get(txn, CURRENT_VIEW_FOR_USER) {
      Some(YrsValue::YMap(map)) => Some(map),
      _ => None,
    };
    match (uid, current_view_for_user_map) {
      (Some(uid), Some(current_view_for_user)) => {
        let view_for_user: Option<String> =
          current_view_for_user.get_with_txn(txn, uid.to_string().as_ref());
        view_for_user
          .and_then(|s| Uuid::parse_str(&s).ok())
          .or_else(|| self.get_child_of_first_public_view(txn, Some(uid)))
      },
      (Some(uid), None) => {
        let current_view: Option<String> = self.meta.get_with_txn(txn, CURRENT_VIEW);
        current_view
          .and_then(|s| Uuid::parse_str(&s).ok())
          .or_else(|| self.get_child_of_first_public_view(txn, Some(uid)))
      },
      (None, _) => {
        let current_view: Option<String> = self.meta.get_with_txn(txn, CURRENT_VIEW);
        current_view.and_then(|s| Uuid::parse_str(&s).ok())
      },
    }
  }

  pub fn set_current_view(&self, txn: &mut TransactionMut, view: ViewId, uid: Option<i64>) {
    if let Some(uid) = uid {
      let current_view_for_user = self.meta.get_or_init_map(txn, CURRENT_VIEW_FOR_USER);
      current_view_for_user.try_update(txn, uid.to_string(), view.to_string());
    }
  }
}

pub fn default_folder_data(uid: i64, workspace_id: &str) -> FolderData {
  let workspace = Workspace {
    id: Uuid::parse_str(workspace_id)
      .unwrap_or_else(|_| crate::entity::uuid_validation::generate_workspace_id()),
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
    current_view: None,
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

  use crate::core::collab::default_client_id;
  use crate::core::{collab::CollabOptions, origin::CollabOrigin};
  use crate::folder::{
    Folder, FolderData, RepeatedViewIdentifier, SectionItem, SpaceInfo, UserId, View,
    ViewIdentifier, ViewLayout, Workspace,
  };
  use crate::preclude::Collab;
  use uuid::Uuid;

  #[test]
  pub fn test_set_and_get_current_view() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = Uuid::parse_str("00000000-0000-0000-0000-000000001234").unwrap();
    let uid = 1;
    let workspace_uuid = workspace_id;
    let options = CollabOptions::new(workspace_uuid, default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let view_1 = View::new(
      Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
      workspace_id,
      "View 1".to_string(),
      ViewLayout::Document,
      Some(uid),
    );
    let view_1_id = view_1.id;
    let view_2 = View::new(
      Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
      workspace_id,
      "View 2".to_string(),
      ViewLayout::Document,
      Some(uid),
    );
    let view_2_id = view_2.id;
    let space_view = View {
      id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
      parent_view_id: Some(workspace_id),
      name: "Space 1".to_string(),
      children: RepeatedViewIdentifier::new(vec![
        ViewIdentifier::new(view_1_id),
        ViewIdentifier::new(view_2_id),
      ]),
      created_at: current_time,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: None,
      last_edited_time: current_time,
      last_edited_by: None,
      is_locked: None,
      extra: Some(serde_json::to_string(&SpaceInfo::default()).unwrap()),
    };
    let space_view_id = space_view.id;
    let workspace = Workspace {
      id: workspace_id,
      name: "Workspace".to_string(),
      child_views: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(space_view_id)]),
      created_at: current_time,
      created_by: Some(uid),
      last_edited_time: current_time,
      last_edited_by: Some(uid),
    };
    let folder_data = FolderData {
      uid,
      workspace,
      current_view: Some(view_2.id),
      views: vec![space_view, view_1, view_2],
      favorites: Default::default(),
      recent: Default::default(),
      trash: Default::default(),
      private: Default::default(),
    };
    let mut folder = Folder::create(collab, None, folder_data);

    folder.set_current_view(view_2_id, uid);
    assert_eq!(folder.get_current_view(uid), Some(view_2_id));
    // First visit from user 2, should return the first child of the first public space with children.
    assert_eq!(folder.get_current_view(2), Some(view_1_id));
    folder.set_current_view(view_1_id, 2);
    assert_eq!(folder.get_current_view(1), Some(view_2_id));
    assert_eq!(folder.get_current_view(2), Some(view_1_id));
  }

  #[test]
  pub fn test_move_section() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = Uuid::parse_str("00000000-0000-0000-0000-000000001234").unwrap();
    let uid = 1;
    let workspace_uuid = workspace_id;
    let options = CollabOptions::new(workspace_uuid, default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let space_view_id = Uuid::parse_str("00000000-0000-0000-0000-000000000010").unwrap();
    let views: Vec<View> = (0..3)
      .map(|i| {
        View::new(
          Uuid::parse_str(&format!("00000000-0000-0000-0000-00000000001{}", i)).unwrap(),
          space_view_id,
          format!("View {:?}", i),
          ViewLayout::Document,
          Some(uid),
        )
      })
      .collect();
    let space_view = View {
      id: space_view_id,
      parent_view_id: None,
      name: "Space".to_string(),
      children: RepeatedViewIdentifier::new(
        views
          .iter()
          .map(|view| ViewIdentifier::new(view.id))
          .collect(),
      ),
      created_at: current_time,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: None,
      last_edited_time: current_time,
      last_edited_by: None,
      is_locked: None,
      extra: Some(serde_json::to_string(&SpaceInfo::default()).unwrap()),
    };
    let workspace = Workspace {
      id: workspace_id,
      name: "Workspace".to_string(),
      child_views: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(space_view_id)]),
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
        views.iter().map(|view| SectionItem::new(view.id)).collect(),
      )]),
      recent: Default::default(),
      trash: Default::default(),
      private: Default::default(),
    };
    let mut folder = Folder::create(collab, None, folder_data);
    let favorite_sections = folder.get_all_favorites_sections(Some(uid));
    // Initially, all 3 views should be in favorites
    assert_eq!(favorite_sections.len(), 3);
    assert_eq!(favorite_sections[0].id, views[0].id);
    assert_eq!(favorite_sections[1].id, views[1].id);
    assert_eq!(favorite_sections[2].id, views[2].id);
    // Move views[0] after views[1]
    folder.move_favorite_view_id(
      &views[0].id.to_string(),
      Some(&views[1].id.to_string()),
      uid,
    );
    let favorite_sections = folder.get_all_favorites_sections(Some(uid));
    // After moving views[0] after views[1], order should be: views[1], views[0], views[2]
    assert_eq!(favorite_sections.len(), 3);
    assert_eq!(favorite_sections[0].id, views[1].id);
    assert_eq!(favorite_sections[1].id, views[0].id);
    assert_eq!(favorite_sections[2].id, views[2].id);
    // Move views[2] to the beginning (None means first position)
    folder.move_favorite_view_id(&views[2].id.to_string(), None, uid);
    let favorite_sections = folder.get_all_favorites_sections(Some(uid));
    // After moving views[2] to the beginning, order should be: views[2], views[1], views[0]
    assert_eq!(favorite_sections.len(), 3);
    assert_eq!(favorite_sections[0].id, views[2].id);
    assert_eq!(favorite_sections[1].id, views[1].id);
    assert_eq!(favorite_sections[2].id, views[0].id);
  }
}
