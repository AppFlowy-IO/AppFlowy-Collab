use crate::{FolderData, SectionsByUid, UserId, View, ViewIcon, ViewLayout};
use std::collections::HashMap;
use uuid::Uuid;

/// A lightweight view representation for hierarchy navigation
#[derive(Debug, Clone)]
pub struct HierarchyView {
  /// The view ID (as UUID)
  pub id: Uuid,
  /// The view name
  pub name: String,
  /// The view icon
  pub icon: Option<ViewIcon>,
  /// The view layout
  pub layout: ViewLayout,
  /// Extra data stored as JSON string
  pub extra: Option<String>,
}

impl HierarchyView {
  /// Create a HierarchyView from a View
  fn from_view(view: &View) -> Option<Self> {
    let id = Uuid::parse_str(&view.id).ok()?;
    Some(Self {
      id,
      name: view.name.clone(),
      icon: view.icon.clone(),
      layout: view.layout.clone(),
      extra: view.extra.clone(),
    })
  }
}

/// A HierarchyView that includes its children
#[derive(Debug, Clone)]
pub struct ViewWithChildren {
  /// The view information
  pub view: HierarchyView,
  /// The immediate children of this view
  pub children: Vec<HierarchyView>,
}

/// A hierarchical representation of folder data that allows efficient navigation
/// between parent and child views, while maintaining section information.
#[derive(Debug, Clone)]
pub struct FolderHierarchy {
  /// The root view (converted from workspace)
  pub root: HierarchyView,

  /// Map from view ID to its parent view ID
  parent_map: HashMap<Uuid, Uuid>,

  /// Map from view ID to list of child view IDs
  children_map: HashMap<Uuid, Vec<Uuid>>,

  /// Map from view ID to the HierarchyView
  view_map: HashMap<Uuid, HierarchyView>,

  /// User ID
  pub uid: i64,

  /// Sections from FolderData
  pub favorites: SectionsByUid,
  pub trash: SectionsByUid,
  pub private: SectionsByUid,
}

impl FolderHierarchy {
  /// Create a new FolderHierarchy from FolderData
  pub fn from_folder_data(folder_data: FolderData) -> Option<Self> {
    // Convert workspace to root view
    let root_view = View::from(folder_data.workspace);
    let root = HierarchyView::from_view(&root_view)?;
    let mut hierarchy = Self {
      root: root.clone(),
      parent_map: HashMap::new(),
      children_map: HashMap::new(),
      view_map: HashMap::new(),
      uid: folder_data.uid,
      favorites: folder_data.favorites,
      trash: folder_data.trash,
      private: folder_data.private,
    };

    // Add root to view map
    hierarchy.view_map.insert(root.id, root.clone());

    // Initialize children map for root
    hierarchy.children_map.insert(root.id, Vec::new());

    // Build the hierarchy from all views
    for view in folder_data.views {
      hierarchy.add_view(view);
    }

    Some(hierarchy)
  }

  /// Add a view to the hierarchy
  fn add_view(&mut self, view: View) {
    // Convert to HierarchyView
    let hierarchy_view = match HierarchyView::from_view(&view) {
      Some(hv) => hv,
      None => return, // Skip views with invalid UUIDs
    };

    let view_uuid = hierarchy_view.id;

    // Add to view map
    self.view_map.insert(view_uuid, hierarchy_view);

    // Update parent map
    if !view.parent_view_id.is_empty() {
      if let Ok(parent_uuid) = Uuid::parse_str(&view.parent_view_id) {
        self.parent_map.insert(view_uuid, parent_uuid);

        // Update children map for parent
        self
          .children_map
          .entry(parent_uuid)
          .or_default()
          .push(view_uuid);
      }
    }

    // Initialize children map for this view (will be populated when its children are added)
    self.children_map.entry(view_uuid).or_default();
  }

  /// Get the parent view ID of a given view
  pub fn get_parent(&self, view_id: &Uuid) -> Option<&Uuid> {
    self.parent_map.get(view_id)
  }

  /// Get the parent view of a given view
  pub fn get_parent_view(&self, view_id: &Uuid) -> Option<&HierarchyView> {
    self
      .parent_map
      .get(view_id)
      .and_then(|parent_id| self.view_map.get(parent_id))
  }

  /// Get all child view IDs of a given view
  pub fn get_children(&self, view_id: &Uuid) -> Option<&Vec<Uuid>> {
    self.children_map.get(view_id)
  }

  /// Get all child views of a given view
  pub fn get_child_views(&self, view_id: &Uuid) -> Vec<&HierarchyView> {
    self
      .children_map
      .get(view_id)
      .map(|children| {
        children
          .iter()
          .filter_map(|child_id| self.view_map.get(child_id))
          .collect()
      })
      .unwrap_or_default()
  }

  /// Get a view by its UUID
  pub fn get_view(&self, view_id: &Uuid) -> Option<&HierarchyView> {
    self.view_map.get(view_id)
  }

  /// Get a view with its children
  pub fn get_view_with_children(&self, view_id: &Uuid) -> Option<ViewWithChildren> {
    let view = self.view_map.get(view_id)?.clone();
    let children = self.get_child_views(view_id).into_iter().cloned().collect();
    Some(ViewWithChildren { view, children })
  }

  /// Get all ancestors of a view (from immediate parent to root)
  pub fn get_ancestors(&self, view_id: &Uuid) -> Vec<&HierarchyView> {
    let mut ancestors = Vec::new();
    let mut current_id = view_id;

    while let Some(parent_id) = self.parent_map.get(current_id) {
      if let Some(parent_view) = self.view_map.get(parent_id) {
        ancestors.push(parent_view);
        current_id = parent_id;
      } else {
        break;
      }
    }

    ancestors
  }

  /// Get all descendants of a view (all children, grandchildren, etc.)
  pub fn get_descendants(&self, view_id: &Uuid) -> Vec<&HierarchyView> {
    let mut descendants = Vec::new();
    let mut stack = vec![*view_id];

    while let Some(current_id) = stack.pop() {
      if let Some(children) = self.children_map.get(&current_id) {
        for child_id in children {
          if let Some(child_view) = self.view_map.get(child_id) {
            descendants.push(child_view);
            stack.push(*child_id);
          }
        }
      }
    }

    descendants
  }

  /// Get the path from root to a specific view
  pub fn get_path_to_view(&self, view_id: &Uuid) -> Vec<&HierarchyView> {
    let mut path = self.get_ancestors(view_id);
    path.reverse(); // Reverse to get root -> ... -> parent order

    // Add the view itself
    if let Some(view) = self.view_map.get(view_id) {
      path.push(view);
    }

    path
  }

  /// Check if a view is in a specific section for the current user
  pub fn is_in_section(&self, view_id: &str, section: FolderSection) -> bool {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    // Only check the current user's sections
    let user_id = UserId::from(self.uid);
    sections
      .get(&user_id)
      .map(|items| items.iter().any(|item| item.id == view_id))
      .unwrap_or(false)
  }

  /// Get all views in a specific section for the current user
  pub fn get_section_views(&self, section: FolderSection) -> Vec<&HierarchyView> {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    // Only get the current user's sections
    let user_id = UserId::from(self.uid);
    let section_items = sections.get(&user_id);

    match section_items {
      Some(items) => items
        .iter()
        .filter_map(|item| {
          Uuid::parse_str(&item.id)
            .ok()
            .and_then(|uuid| self.view_map.get(&uuid))
        })
        .collect(),
      None => Vec::new(),
    }
  }

  /// Get siblings of a view (other children of the same parent)
  pub fn get_siblings(&self, view_id: &Uuid) -> Vec<&HierarchyView> {
    self
      .parent_map
      .get(view_id)
      .and_then(|parent_id| self.children_map.get(parent_id))
      .map(|siblings| {
        siblings
          .iter()
          .filter(|&sibling_id| sibling_id != view_id)
          .filter_map(|sibling_id| self.view_map.get(sibling_id))
          .collect()
      })
      .unwrap_or_default()
  }

  /// Check if a view is an ancestor of another view
  pub fn is_ancestor_of(&self, ancestor_id: &Uuid, descendant_id: &Uuid) -> bool {
    let mut current_id = descendant_id;

    while let Some(parent_id) = self.parent_map.get(current_id) {
      if parent_id == ancestor_id {
        return true;
      }
      current_id = parent_id;
    }

    false
  }

  /// Get the depth of a view (distance from root)
  pub fn get_depth(&self, view_id: &Uuid) -> usize {
    self.get_ancestors(view_id).len()
  }

  /// Get all views in the hierarchy as a flat list
  pub fn get_all_views(&self) -> Vec<&HierarchyView> {
    self.view_map.values().collect()
  }

  /// Get all descendants with their children up to a specified depth
  /// depth = 0: no descendants
  /// depth = 1: only direct children  
  /// depth = 2: children and grandchildren, etc.
  pub fn get_descendants_with_children(
    &self,
    view_id: &Uuid,
    max_depth: usize,
  ) -> Vec<ViewWithChildren> {
    if max_depth == 0 {
      return vec![];
    }

    let mut result = Vec::new();
    let mut queue = Vec::new();

    // Start with direct children at depth 1
    if let Some(children) = self.children_map.get(view_id) {
      for child_id in children {
        queue.push((*child_id, 1));
      }
    }

    while let Some((current_id, current_depth)) = queue.pop() {
      if let Some(view) = self.view_map.get(&current_id) {
        let children = self
          .get_child_views(&current_id)
          .into_iter()
          .cloned()
          .collect();

        result.push(ViewWithChildren {
          view: view.clone(),
          children,
        });

        // Add children to queue if we haven't reached max depth
        if current_depth < max_depth {
          if let Some(children) = self.children_map.get(&current_id) {
            for child_id in children {
              queue.push((*child_id, current_depth + 1));
            }
          }
        }
      }
    }

    result
  }

  /// Get the path from root to a specific view with children
  pub fn get_path_to_view_with_children(&self, view_id: &Uuid) -> Vec<ViewWithChildren> {
    self
      .get_path_to_view(view_id)
      .into_iter()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewWithChildren {
          view: view.clone(),
          children,
        }
      })
      .collect()
  }

  /// Get all views in a specific section with their children
  pub fn get_section_views_with_children(
    &self,
    section: FolderSection,
  ) -> Vec<ViewWithChildren> {
    self
      .get_section_views(section)
      .into_iter()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewWithChildren {
          view: view.clone(),
          children,
        }
      })
      .collect()
  }

  /// Get all views in the hierarchy with their children
  pub fn get_all_views_with_children(&self) -> Vec<ViewWithChildren> {
    self
      .view_map
      .values()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewWithChildren {
          view: view.clone(),
          children,
        }
      })
      .collect()
  }
}

/// Enum to represent different folder sections
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderSection {
  Favorites,
  Trash,
  Private,
}
