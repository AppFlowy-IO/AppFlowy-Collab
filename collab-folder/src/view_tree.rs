use crate::{FolderData, SectionsByUid, View, ViewIcon, ViewLayout};
use std::collections::HashMap;
use uuid::Uuid;

/// Map of user IDs to their section view IDs
pub type UserSectionViews = HashMap<i64, Vec<Uuid>>;

/// Convert SectionsByUid to UserSectionViews
pub fn convert_sections_to_uuids(sections: SectionsByUid) -> UserSectionViews {
  sections
    .into_iter()
    .map(|(user_id, items)| {
      let uuids: Vec<Uuid> = items
        .into_iter()
        .filter_map(|item| Uuid::parse_str(&item.id).ok())
        .collect();
      (user_id.as_i64(), uuids)
    })
    .collect()
}

/// Lightweight representation of a view for tree operations
#[derive(Debug, Clone)]
pub struct ViewNode {
  /// Unique identifier
  pub id: Uuid,
  /// View name
  pub name: String,
  /// Optional icon
  pub icon: Option<ViewIcon>,
  /// View layout type
  pub layout: ViewLayout,
  /// Optional extra data
  pub extra: Option<String>,
}

impl ViewNode {
  /// Create a ViewNode from a View
  fn from_view(view: &View) -> Result<Self, uuid::Error> {
    Ok(ViewNode {
      id: Uuid::parse_str(&view.id)?,
      name: view.name.clone(),
      icon: view.icon.clone(),
      layout: view.layout.clone(),
      extra: view.extra.clone(),
    })
  }
}

/// A ViewNode that includes its children
#[derive(Debug, Clone)]
pub struct ViewNodeWithChildren {
  /// The view information
  pub view: ViewNode,
  /// The immediate children of this view
  pub children: Vec<ViewNode>,
}

/// Available sections in the tree
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderSection {
  Favorites,
  Trash,
  Private,
}

/// Tree structure for efficient view navigation and queries
pub struct FolderTree {
  /// Root view (workspace)
  pub root: ViewNode,

  /// Maps view ID to its children IDs
  children_map: HashMap<Uuid, Vec<Uuid>>,

  /// Maps view ID to its ViewNode
  view_map: HashMap<Uuid, ViewNode>,

  /// Sections from FolderData
  pub favorites: UserSectionViews,
  pub trash: UserSectionViews,
  pub private: UserSectionViews,
}

impl FolderTree {
  pub fn from_data(root: View, favorites: UserSectionViews, trash: UserSectionViews, private: UserSectionViews, views: Vec<View>) -> Result<Self, uuid::Error> {
    let root_node = ViewNode::from_view(&root)?;

    let mut tree = Self {
      root: root_node.clone(),
      children_map: HashMap::new(),
      view_map: HashMap::new(),
      favorites,
      trash,
      private,
    };

    // Add root to view map
    tree.view_map.insert(root_node.id, root_node.clone());

    // Initialize root's children
    tree.children_map.insert(root_node.id, Vec::new());

    // Add the root view itself
    for view in views {
      tree.add_view(view)?;
    }

    Ok(tree)
  }

  /// Create a ViewTree from FolderData
  pub fn from_folder_data(folder_data: FolderData) -> Result<Self, uuid::Error> {
    let root_view = View::from(folder_data.workspace);
    let favorites = convert_sections_to_uuids(folder_data.favorites);
    let trash = convert_sections_to_uuids(folder_data.trash);
    let private = convert_sections_to_uuids(folder_data.private);
    Self::from_data(root_view, favorites, trash, private, folder_data.views)
  }

  /// Add a view to the tree
  fn add_view(&mut self, view: View) -> Result<(), uuid::Error> {
    let view_uuid = Uuid::parse_str(&view.id)?;
    let parent_uuid = Uuid::parse_str(&view.parent_view_id)?;

    // Create ViewNode
    let view_node = ViewNode::from_view(&view)?;

    // Add to view map
    self.view_map.insert(view_uuid, view_node);

    // Add to parent's children
    self
      .children_map
      .entry(parent_uuid)
      .or_default()
      .push(view_uuid);

    // Initialize children map for this view (will be populated when its children are added)
    self.children_map.entry(view_uuid).or_default();

    Ok(())
  }

  /// Find parent ID by searching through children_map
  fn find_parent_id(&self, view_id: &Uuid) -> Option<Uuid> {
    for (parent_id, children) in &self.children_map {
      if children.contains(view_id) {
        return Some(*parent_id);
      }
    }
    None
  }

  /// Get a view by ID
  pub fn get_view(&self, view_id: &Uuid) -> Option<&ViewNode> {
    self.view_map.get(view_id)
  }

  /// Get parent ID of a view
  pub fn get_parent(&self, view_id: &Uuid) -> Option<Uuid> {
    self.find_parent_id(view_id)
  }

  /// Get parent view
  pub fn get_parent_view(&self, view_id: &Uuid) -> Option<&ViewNode> {
    self
      .find_parent_id(view_id)
      .and_then(|parent_id| self.view_map.get(&parent_id))
  }

  /// Get children IDs of a view
  pub fn get_children(&self, view_id: &Uuid) -> Option<&Vec<Uuid>> {
    self.children_map.get(view_id)
  }

  /// Get child views
  pub fn get_child_views(&self, view_id: &Uuid) -> Vec<&ViewNode> {
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

  /// Get a view with its children
  pub fn get_view_with_children(&self, view_id: &Uuid) -> Option<ViewNodeWithChildren> {
    let view = self.view_map.get(view_id)?.clone();
    let children = self.get_child_views(view_id).into_iter().cloned().collect();
    Some(ViewNodeWithChildren { view, children })
  }

  /// Get all ancestors of a view (from immediate parent to root)
  /// Includes cycle detection to prevent infinite loops
  pub fn get_ancestors(&self, view_id: &Uuid) -> Vec<&ViewNode> {
    let mut ancestors = Vec::new();
    let mut current_id = *view_id;
    let mut visited = std::collections::HashSet::new();
    let max_depth = 1000; // Prevent infinite loops

    for _ in 0..max_depth {
      if visited.contains(&current_id) {
        // Cycle detected, break to prevent infinite loop
        break;
      }
      visited.insert(current_id);

      if let Some(parent_id) = self.find_parent_id(&current_id) {
        if let Some(parent_view) = self.view_map.get(&parent_id) {
          ancestors.push(parent_view);
          current_id = parent_id;
        } else {
          break;
        }
      } else {
        break;
      }
    }

    ancestors
  }

  /// Get all descendants of a view (all children, grandchildren, etc.)
  pub fn get_descendants(&self, view_id: &Uuid) -> Vec<&ViewNode> {
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
  pub fn get_path_to_view(&self, view_id: &Uuid) -> Vec<&ViewNode> {
    let mut path = self.get_ancestors(view_id);
    path.reverse(); // Reverse to get root -> ... -> parent order

    // Add the view itself
    if let Some(view) = self.view_map.get(view_id) {
      path.push(view);
    }

    path
  }

  /// Check if a view is in a specific section for a given user
  pub fn is_in_section(&self, view_id: &str, section: FolderSection, user_id: i64) -> bool {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    // Parse view_id to Uuid
    let view_uuid = match Uuid::parse_str(view_id) {
      Ok(uuid) => uuid,
      Err(_) => return false,
    };

    // Check the specified user's sections
    sections
      .get(&user_id)
      .map(|uuids| uuids.contains(&view_uuid))
      .unwrap_or(false)
  }

  /// Get all views in a specific section for a given user
  pub fn get_section_views(&self, section: FolderSection, user_id: i64) -> Vec<&ViewNode> {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    // Get the specified user's sections
    let section_uuids = sections.get(&user_id);

    match section_uuids {
      Some(uuids) => uuids
        .iter()
        .filter_map(|uuid| self.view_map.get(uuid))
        .collect(),
      None => Vec::new(),
    }
  }

  /// Get siblings of a view (other children of the same parent)
  pub fn get_siblings(&self, view_id: &Uuid) -> Vec<&ViewNode> {
    self
      .find_parent_id(view_id)
      .and_then(|parent_id| self.children_map.get(&parent_id))
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
  /// Includes cycle detection to prevent infinite loops
  pub fn is_ancestor_of(&self, ancestor_id: &Uuid, descendant_id: &Uuid) -> bool {
    let mut current_id = *descendant_id;
    let mut visited = std::collections::HashSet::new();
    let max_depth = 1000; // Prevent infinite loops

    for _ in 0..max_depth {
      if visited.contains(&current_id) {
        // Cycle detected, break to prevent infinite loop
        break;
      }
      visited.insert(current_id);

      if let Some(parent_id) = self.find_parent_id(&current_id) {
        if &parent_id == ancestor_id {
          return true;
        }
        current_id = parent_id;
      } else {
        break;
      }
    }

    false
  }

  /// Get the depth of a view (distance from root)
  /// Includes cycle detection to prevent infinite loops
  pub fn get_depth(&self, view_id: &Uuid) -> usize {
    let mut depth = 0;
    let mut current_id = *view_id;
    let mut visited = std::collections::HashSet::new();
    let max_depth = 1000; // Prevent infinite loops

    for _ in 0..max_depth {
      if visited.contains(&current_id) {
        // Cycle detected, return current depth
        break;
      }
      visited.insert(current_id);

      if let Some(parent_id) = self.find_parent_id(&current_id) {
        depth += 1;
        current_id = parent_id;
      } else {
        break;
      }
    }

    depth
  }

  /// Check if the tree contains any cycles
  pub fn has_cycles(&self) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut rec_stack = std::collections::HashSet::new();

    for view_id in self.view_map.keys() {
      if !visited.contains(view_id) && self.has_cycle_dfs(view_id, &mut visited, &mut rec_stack) {
        return true;
      }
    }
    false
  }

  /// DFS helper to detect cycles
  fn has_cycle_dfs(
    &self,
    view_id: &Uuid,
    visited: &mut std::collections::HashSet<Uuid>,
    rec_stack: &mut std::collections::HashSet<Uuid>,
  ) -> bool {
    visited.insert(*view_id);
    rec_stack.insert(*view_id);

    if let Some(children) = self.children_map.get(view_id) {
      for child_id in children {
        if !visited.contains(child_id) {
          if self.has_cycle_dfs(child_id, visited, rec_stack) {
            return true;
          }
        } else if rec_stack.contains(child_id) {
          return true;
        }
      }
    }

    rec_stack.remove(view_id);
    false
  }

  /// Get all views in the tree as a flat list
  pub fn get_all_views(&self) -> Vec<&ViewNode> {
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
  ) -> Vec<ViewNodeWithChildren> {
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

        result.push(ViewNodeWithChildren {
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
  pub fn get_path_to_view_with_children(&self, view_id: &Uuid) -> Vec<ViewNodeWithChildren> {
    self
      .get_path_to_view(view_id)
      .into_iter()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewNodeWithChildren {
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
    user_id: i64,
  ) -> Vec<ViewNodeWithChildren> {
    self
      .get_section_views(section, user_id)
      .into_iter()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewNodeWithChildren {
          view: view.clone(),
          children,
        }
      })
      .collect()
  }

  /// Get all views in the tree with their children
  pub fn get_all_views_with_children(&self) -> Vec<ViewNodeWithChildren> {
    self
      .view_map
      .values()
      .map(|view| {
        let children = self
          .get_child_views(&view.id)
          .into_iter()
          .cloned()
          .collect();
        ViewNodeWithChildren {
          view: view.clone(),
          children,
        }
      })
      .collect()
  }

  /// Get all user IDs that have views in a specific section
  pub fn get_section_user_ids(&self, section: FolderSection) -> Vec<i64> {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    sections.keys().cloned().collect()
  }

  /// Get all views in a specific section for all users
  pub fn get_all_users_section_views(
    &self,
    section: FolderSection,
  ) -> HashMap<i64, Vec<&ViewNode>> {
    let sections = match section {
      FolderSection::Favorites => &self.favorites,
      FolderSection::Trash => &self.trash,
      FolderSection::Private => &self.private,
    };

    sections
      .iter()
      .map(|(&user_id, uuids)| {
        let views: Vec<&ViewNode> = uuids
          .iter()
          .filter_map(|uuid| self.view_map.get(uuid))
          .collect();
        (user_id, views)
      })
      .collect()
  }
}
