use super::view::{FolderState, ViewData, ViewPatch};
use crate::error::FolderError;
use crate::hierarchy_builder::ParentChildViews;
use crate::section::{Section, SectionItem};
use crate::v2::fractional_index::{FractionalVec, index_between};
use crate::v2::provider::FolderDataProvider;
use crate::{
  FolderData, FolderNotify, RepeatedViewIdentifier, SectionChange, TrashInfo, TrashSectionChange,
  ViewChange, ViewId, ViewIdentifier, Workspace,
};
use anyhow::anyhow;
use collab::core::collab::DataSource;
pub use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use std::collections::hash_map::Entry;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

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
  data: FolderState,
  provider: Box<dyn FolderDataProvider>,
  notifier: Option<FolderNotify>,
}

impl Folder {
  pub fn open(
    provider: Box<dyn FolderDataProvider>,
    workspace_id: String,
    notifier: Option<FolderNotify>,
  ) -> Self {
    Folder {
      data: FolderState::new(workspace_id),
      provider,
      notifier,
    }
  }

  pub async fn create(
    provider: Box<dyn FolderDataProvider>,
    notifier: Option<FolderNotify>,
    data: FolderData,
  ) -> super::Result<Self> {
    let state = FolderState::from(data);
    provider.init(&state).await?;
    Ok(Folder {
      data: state,
      provider,
      notifier,
    })
  }

  pub fn from_collab_doc_state(
    origin: CollabOrigin,
    collab_doc_state: DataSource,
    workspace_id: &str,
    client_id: ClientID,
  ) -> super::Result<Self> {
    //let options =
    //  CollabOptions::new(workspace_id.to_string(), client_id).with_data_source(collab_doc_state);
    //let collab = Collab::new_with_options(origin, options)?;
    //Self::open(collab, None)
    todo!()
  }

  pub fn get_current_view(&self, uid: i64) -> Option<ViewId> {
    self.data.current_views.get(&uid).cloned()
  }

  pub fn set_current_view(&mut self, view: ViewId, uid: i64) {
    match self.data.current_views.entry(uid) {
      Entry::Occupied(mut e) => {
        e.insert(view);
      },
      Entry::Vacant(e) => {
        e.insert(view);
      },
    };
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
  pub fn get_folder_data(&self, workspace_id: &str, uid: i64) -> Option<&FolderState> {
    if &*self.data.workspace_id != workspace_id {
      return None;
    }
    Some(&self.data)
  }

  /// Fetches the current workspace.
  ///
  /// This function fetches the ID of the current workspace from the metaobject,
  /// and uses this ID to fetch the actual workspace object.
  ///
  pub fn get_workspace_info(&self, workspace_id: &str, uid: i64) -> Option<Workspace> {
    if &*self.data.workspace_id != workspace_id {
      return None;
    }
    Some(self.data.workspace())
  }

  pub fn get_workspace_id(&self) -> ViewId {
    self.data.workspace_id.clone()
  }

  pub fn get_all_views(&self, uid: i64) -> Vec<&ViewData> {
    self.data.views.values().collect()
  }

  pub fn get_views<T: AsRef<str>>(&self, view_ids: &[T], uid: i64) -> Vec<&ViewData> {
    let view_ids: HashSet<_> = view_ids.iter().map(|id| id.as_ref()).collect();
    self
      .data
      .views
      .values()
      .filter(|v| view_ids.contains(&*v.id))
      .collect()
  }

  pub fn get_views_belong_to(&self, parent_id: &str, uid: i64) -> Vec<crate::View> {
    let children = self.child_views(parent_id);
    let views: Vec<crate::View> = children
      .iter()
      .filter_map(|id| self.get_view(id, uid))
      .collect();
    views
  }

  pub async fn move_view(
    &mut self,
    view_id: &str,
    _from: u32,
    to: u32,
    uid: i64,
  ) -> super::Result<crate::View> {
    let view = self.data.views.get(view_id).ok_or_else(|| {
      FolderError::NoRequiredData(format!("View {} not found when moving", view_id))
    })?;
    let frac_index = self
      .child_views(&view.parent_view_id)
      .index_at(Some(to as usize));

    let view = self.data.views.get_mut(view_id).unwrap();

    view.parent_ordering = frac_index.clone();

    let mut patch = ViewPatch::new(view.id.clone());
    patch.parent_view_id = Some(view.parent_view_id.clone());
    patch.parent_ordering = Some(frac_index);

    self.provider.update_view(patch).await?;

    let mut result: crate::View = view.clone().into();
    result.children = self.child_ids(view_id);
    result.is_favorite = self.is_favorite(view_id, uid);

    if let Some(notify) = &self.notifier {
      let _ = notify.view_change_tx.send(ViewChange::DidUpdate {
        view: result.clone(),
      });
    }

    Ok(result)
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
  pub async fn move_nested_view(
    &mut self,
    view_id: &str,
    new_parent_id: &str,
    prev_view_id: Option<String>,
    uid: i64,
  ) -> super::Result<crate::View> {
    let syblings = self.child_views(new_parent_id);
    let (left, right) = syblings.neighbors_after(|v| v.as_ref() == view_id);
    let frac_index = index_between(left, right).unwrap();

    let view = self.data.views.get_mut(view_id).ok_or_else(|| {
      FolderError::NoRequiredData(format!("View {} not found when moving", view_id))
    })?;

    view.parent_view_id = new_parent_id.into();
    view.parent_ordering = frac_index.clone();

    let mut patch = ViewPatch::new(view_id.into());
    patch.parent_view_id = Some(new_parent_id.into());
    patch.parent_ordering = Some(frac_index);
    self.provider.update_view(patch).await?;

    let mut result: crate::View = view.clone().into();
    result.children = self.child_ids(view_id);
    result.is_favorite = self.is_favorite(view_id, uid);

    if let Some(notify) = &self.notifier {
      let _ = notify.view_change_tx.send(ViewChange::DidUpdate {
        view: result.clone(),
      });
    }

    Ok(result)
  }

  pub async fn update_view<F>(&mut self, view_id: &str, f: F, uid: i64) -> Option<crate::View>
  where
    F: FnOnce(&mut ViewData),
  {
    let view = match self.data.views.get_mut(view_id) {
      Some(view) => view,
      None => return None,
    };
    let mut new_view = view.clone();
    f(&mut new_view);
    if let Some(patch) = view.create_patch(&new_view) {
      self.provider.update_view(patch).await.ok()?;
    }
    *view = new_view.clone();

    let mut result: crate::View = view.clone().into();
    result.children = self.child_ids(view_id);
    result.is_favorite = self.is_favorite(view_id, uid);

    if let Some(notifier) = &self.notifier {
      let _ = notifier.view_change_tx.send(ViewChange::DidUpdate {
        view: result.clone(),
      });
    }

    Some(result)
  }

  pub async fn delete_views(&mut self, views: &[ViewId]) -> super::Result<()> {
    self.provider.delete_views(views).await?;
    let mut deleted = Vec::with_capacity(views.len());
    for view_id in views {
      if let Some(view) = self.data.views.remove(view_id) {
        deleted.push(Arc::new(crate::View::from(view)));
      }
    }

    if let Some(notifier) = &self.notifier {
      let _ = notifier
        .view_change_tx
        .send(ViewChange::DidDeleteView { views: deleted });
    }

    Ok(())
  }

  /// Add view IDs as either favorites or recents
  pub fn section_add(&mut self, section: Section, ids: &[ViewId], uid: i64) {
    let e = self.data.sections.entry(section.clone()).or_default();
    let by_user = e.entry(uid).or_default();

    by_user.append(ids.into_iter().map(|id| SectionItem::new(id.clone())));

    if section == Section::Trash {
      if let Some(notifier) = &self.notifier {
        let _ = notifier.section_change_tx.send(SectionChange::Trash(
          TrashSectionChange::TrashItemAdded { ids: ids.to_vec() },
        ));
      }
    }

    todo!("persistence");
  }

  pub fn section_delete(&mut self, section: &Section, ids: &[ViewId], uid: i64) {
    if let Some(by_user) = self.data.sections.get_mut(section) {
      if let Some(section_items) = by_user.get_mut(&uid) {
        let id_set: HashSet<_> = ids.iter().collect();
        section_items.remove_all(|v| !id_set.contains(&&v.id));
      }
    }

    if section == &Section::Trash {
      if let Some(notifier) = &self.notifier {
        let _ = notifier.section_change_tx.send(SectionChange::Trash(
          TrashSectionChange::TrashItemRemoved { ids: ids.to_vec() },
        ));
      }
    }

    todo!("persistence");
  }

  // Get all section items for the current user
  pub fn section_get(&self, section: &Section, uid: i64) -> Vec<SectionItem> {
    let mut result = Vec::new();
    if let Some(by_user) = self.data.sections.get(section) {
      if let Some(section_items) = by_user.get(&uid) {
        result.extend(section_items.iter().cloned());
      }
    }
    result
  }

  // Get all sections
  pub fn section_get_all(&self, section: &Section, uid: i64) -> Vec<SectionItem> {
    let mut result = Vec::new();
    if let Some(by_user) = self.data.sections.get(section) {
      for section_items in by_user.values() {
        result.extend(section_items.iter().cloned());
      }
    }
    result
  }

  // Clear all items in a section
  pub fn section_remove_all(&mut self, section: &Section, uid: i64) {
    if let Some(by_user) = self.data.sections.get_mut(section) {
      by_user.remove(&uid);
    }
    //TODO: notify changes

    todo!("persistence");
  }

  // Move the position of a single section item to after another section item. If
  // prev_id is None, the item will be moved to the beginning of the section.
  pub fn section_move(&mut self, section: &Section, id: &str, prev_id: Option<&str>, uid: i64) {
    if let Some(by_user) = self.data.sections.get_mut(section) {
      if let Some(section_items) = by_user.get_mut(&uid) {
        section_items.insert_after(SectionItem::new(id.into()), |i| {
          Some(i.id.as_ref()) == prev_id
        });
      }
    }
    //TODO: notify changes

    todo!("persistence");
  }

  pub fn get_my_trash_info(&self, uid: i64) -> Vec<TrashInfo> {
    let views = match self.data.sections.get(&Section::Trash) {
      Some(by_user) => match by_user.get(&uid) {
        Some(section) => section,
        None => return vec![],
      },
      None => return vec![],
    };
    views
      .iter()
      .filter_map(|item| {
        self.data.views.get(&item.id).map(|view| TrashInfo {
          id: view.id.clone(),
          name: view.name.clone(),
          created_at: view.created_at,
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
  pub async fn insert_view(
    &mut self,
    view: crate::View,
    index: Option<usize>,
    uid: i64,
  ) -> super::Result<()> {
    let syblings = self.child_views(&view.parent_view_id);
    let frac_index = syblings.index_at(index);
    let mut view: ViewData = view.into();
    view.parent_ordering = frac_index;
    self.provider.insert_views(&[view.clone()], uid).await?;
    self.data.views.insert(view.id.clone(), view);
    //TODO: notify changes
    Ok(())
  }

  fn child_views(&self, parent_id: &str) -> FractionalVec<ViewId> {
    self
      .data
      .views
      .iter()
      .filter_map(|(id, view)| {
        if &*view.parent_view_id == parent_id {
          Some((view.parent_ordering.clone(), id.clone()))
        } else {
          None
        }
      })
      .collect()
  }

  /// Insert a list of views at the end of its parent view
  pub async fn insert_views(&mut self, views: Vec<crate::View>, uid: i64) -> super::Result<()> {
    if views.is_empty() {
      return Ok(());
    }
    let mut left_frac_index = None;
    let mut parent_id = None;
    let mut result = Vec::with_capacity(views.len());
    for view in views {
      let mut view = ViewData::from(view);

      // calculate the fractional index

      // get index of the left neighbor (right neighbor is always None since we are appending to the end)
      let left = if parent_id.as_ref() == Some(&view.parent_view_id) {
        left_frac_index // view has the same parent as the previous one
      } else {
        self
          .data
          .views
          .iter()
          .filter(|(_, v)| v.parent_view_id == view.parent_view_id)
          .map(|(_, v)| v.parent_ordering.clone())
          .max()
      };
      // cache left index for the next view
      let index = index_between(left.as_ref(), None).unwrap();
      left_frac_index = Some(index.clone());

      parent_id = Some(view.parent_view_id.clone());
      view.parent_ordering = index;

      result.push(view);
    }

    self.provider.insert_views(&result, uid).await?;
    for view in result {
      self.data.views.insert(view.id.clone(), view);
    }
    //TODO: notify changes
    Ok(())
  }

  /// Insert parent-children views into the folder.
  /// when only insert one view, user [Self::insert_view] instead.
  pub async fn insert_nested_views(
    &mut self,
    views: Vec<ParentChildViews>,
    uid: i64,
  ) -> super::Result<()> {
    fn flatten_views(views: Vec<ParentChildViews>, flattened: &mut Vec<ViewData>) {
      let mut left = None;
      for view in views {
        let mut data = ViewData::from(view.view);
        data.parent_ordering = index_between(left.as_ref(), None).unwrap();
        left = Some(data.parent_ordering.clone());
        flattened.push(data);

        if !view.children.is_empty() {
          flatten_views(view.children, flattened);
        }
      }
    }

    let mut flattened = Vec::new();
    flatten_views(views, &mut flattened);

    self.provider.insert_views(&flattened, uid).await?;
    for view in flattened {
      self.data.views.insert(view.id.clone(), view);
    }
    //TODO: notify changes
    Ok(())
  }

  pub fn get_view(&self, view_id: &str, uid: i64) -> Option<crate::View> {
    let view = self.data.views.get(view_id)?;
    let mut result: crate::View = view.clone().into();
    result.children = self.child_ids(view_id);
    result.is_favorite = self.is_favorite(view_id, uid);

    Some(result)
  }

  fn child_ids(&self, parent_id: &str) -> RepeatedViewIdentifier {
    RepeatedViewIdentifier::new(
      self
        .child_views(parent_id)
        .iter()
        .map(|v| ViewIdentifier::new(v.clone()))
        .collect(),
    )
  }

  fn is_favorite(&self, view_id: &str, uid: i64) -> bool {
    self
      .data
      .sections
      .get(&Section::Favorite)
      .and_then(|by_user| by_user.get(&uid))
      .map(|section| section.iter().any(|item| &*item.id == view_id))
      .unwrap_or(false)
  }

  pub fn is_view_in_section(&self, section: Section, view_id: &str, uid: i64) -> bool {
    self
      .data
      .sections
      .get(&section)
      .and_then(|by_user| by_user.get(&uid))
      .map(|section| section.iter().any(|item| item.id.as_ref() == view_id))
      .unwrap_or(false)
  }

  pub fn to_json(&self) -> String {
    self.to_json_value().to_string()
  }

  pub fn to_json_value(&self) -> JsonValue {
    let data = self.get_folder_data(&self.get_workspace_id(), 0).unwrap();
    serde_json::to_value(data).unwrap()
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
  pub fn get_view_recursively(&self, view_id: &str, uid: i64) -> Vec<crate::View> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    // breadth-first scan queue
    let mut queue = VecDeque::new();

    let mut current_children = Vec::new();

    let view = match self.get_view(view_id, uid) {
      Some(view) => view,
      None => return result,
    };
    queue.push_back(view);
    while let Some(view) = queue.pop_front() {
      if !visited.insert(view.id.clone()) {
        // deduplication check
        continue;
      }

      // collect children of the current view and sort them in order
      for (_, child) in self.data.views.iter() {
        if child.parent_view_id == view.id {
          current_children.push(child);
        }
      }
      current_children.sort_by_key(|v| &*v.parent_ordering);

      result.push(view);

      // push children to the processing queue
      queue.extend(current_children.drain(..).map(|v| {
        let mut view: crate::View = v.clone().into();
        view.children = self.child_ids(&v.id);
        view.is_favorite = self.is_favorite(&v.id, uid);
        view
      }));
    }

    result
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::{Folder, SectionItem, ViewId, Workspace};
  use crate::v2::provider::NoopFolderDataProvider;
  use crate::{
    FolderData, RepeatedViewIdentifier, Section, SpaceInfo, UserId, View, ViewIdentifier,
  };
  use collab::core::collab::default_client_id;
  use collab::{core::collab::CollabOptions, core::origin::CollabOrigin, preclude::Collab};

  #[tokio::test]
  pub async fn test_set_and_get_current_view() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = "1234";
    let uid = 1;
    let options = CollabOptions::new(workspace_id.to_string(), default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let view_1 = View::new(
      "view_1".into(),
      workspace_id.into(),
      "View 1".into(),
      crate::ViewLayout::Document,
      Some(uid),
    );
    let view_1_id = view_1.id.clone();
    let view_2 = View::new(
      "view_2".into(),
      workspace_id.into(),
      "View 2".into(),
      crate::ViewLayout::Document,
      Some(uid),
    );
    let view_2_id = view_2.id.clone();
    let space_view = View {
      id: "space_1_id".into(),
      parent_view_id: workspace_id.into(),
      name: "Space 1".into(),
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
      id: workspace_id.into(),
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
    let provider = Box::new(NoopFolderDataProvider);
    let mut folder = Folder::create(provider, None, folder_data).await.unwrap();

    folder.set_current_view(view_2_id.clone(), uid);
    assert_eq!(folder.get_current_view(uid), Some(view_2_id.clone()));
    // First visit from user 2, should return the first child of the first public space with children.
    assert_eq!(folder.get_current_view(2), Some(view_1_id.clone()));
    folder.set_current_view(view_1_id.clone(), 2);
    assert_eq!(folder.get_current_view(1), Some(view_2_id));
    assert_eq!(folder.get_current_view(2), Some(view_1_id));
  }

  #[tokio::test]
  pub async fn test_move_section() {
    let current_time = chrono::Utc::now().timestamp();
    let workspace_id = "1234";
    let uid = 1;
    let options = CollabOptions::new(workspace_id.to_string(), default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let space_view_id: ViewId = "space_view_id".into();
    let views: Vec<_> = (0..3)
      .map(|i| {
        View::new(
          format!("view_{:?}", i).into(),
          space_view_id.clone(),
          format!("View {:?}", i),
          crate::ViewLayout::Document,
          Some(uid),
        )
      })
      .collect();
    let space_view = View {
      id: "space_1_id".into(),
      parent_view_id: workspace_id.into(),
      name: "Space 1".into(),
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
      id: workspace_id.into(),
      name: "Workspace".to_string(),
      child_views: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(space_view_id.clone())]),
      created_at: current_time,
      created_by: Some(uid),
      last_edited_time: current_time,
      last_edited_by: Some(uid),
    };
    let all_views: Vec<_> = views
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
    let provider = Box::new(NoopFolderDataProvider);
    let mut folder = Folder::create(provider, None, folder_data).await.unwrap();
    let favorite_sections = folder.section_get_all(&Section::Favorite, uid);
    let expected_favorites = vec![
      SectionItem::new("view_0".into()),
      SectionItem::new("view_1".into()),
      SectionItem::new("view_2".into()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
    folder.section_move(&Section::Favorite, "view_0", Some("view_1"), uid);
    let favorite_sections = folder.section_get_all(&Section::Favorite, uid);
    let expected_favorites = vec![
      SectionItem::new("view_1".into()),
      SectionItem::new("view_0".into()),
      SectionItem::new("view_2".into()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
    folder.section_move(&Section::Favorite, "view_2", None, uid);
    let favorite_sections = folder.section_get_all(&Section::Favorite, uid);
    let expected_favorites = vec![
      SectionItem::new("view_2".into()),
      SectionItem::new("view_1".into()),
      SectionItem::new("view_0".into()),
    ];
    assert_eq!(favorite_sections, expected_favorites);
  }
}
