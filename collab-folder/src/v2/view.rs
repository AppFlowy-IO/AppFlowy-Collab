use crate::v2::folder::FractionalVec;
use crate::v2::fractional_index::FractionalIndex;
use crate::{Section, SectionItem, ViewIcon, ViewId, ViewIdentifier, ViewLayout};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Workspace {
  pub id: ViewId,
  pub name: String,
  pub child_views: FractionalVec<ViewIdentifier>,
  pub created_at: i64,
  pub created_by: Option<i64>,
  pub last_edited_time: i64,
  pub last_edited_by: Option<i64>,
}

impl Workspace {
  pub fn new(id: ViewId) -> Self {
    Workspace {
      id,
      name: "".into(),
      child_views: Default::default(),
      created_at: 0,
      created_by: None,
      last_edited_time: 0,
      last_edited_by: None,
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FolderData {
  pub workspace: Workspace,
  pub current_views: HashMap<i64, ViewId>,
  pub views: HashMap<ViewId, View>,
  pub sections: HashMap<Section, HashMap<i64, FractionalVec<SectionItem>>>,
}

impl FolderData {
  pub fn new(workspace: Workspace) -> Self {
    FolderData {
      current_views: HashMap::new(),
      workspace,
      views: HashMap::new(),
      sections: HashMap::new(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ParentChildViews {
  pub view: View,
  pub children: Vec<ParentChildViews>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct View {
  /// The id of the view
  pub id: ViewId,
  /// The id for given parent view
  pub parent_view_id: ViewId,
  /// A list of ids, each of them is the id of other view
  pub parent_ordering: FractionalIndex,
  /// The name that display on the left sidebar
  pub name: String,
  pub created_at: i64,
  #[serde(default)]
  pub is_favorite: bool,
  pub layout: ViewLayout,
  pub icon: Option<ViewIcon>,
  pub created_by: Option<i64>, // user id
  pub last_edited_time: i64,
  pub last_edited_by: Option<i64>, // user id
  pub is_locked: Option<bool>,
  /// this value used to store the extra data with JSON format
  /// for document:
  /// - cover: { type: "", value: "" }
  ///   - type: "0" represents normal color,
  ///           "1" represents gradient color,
  ///           "2" represents built-in image,
  ///           "3" represents custom image,
  ///           "4" represents local image,
  ///           "5" represents unsplash image
  /// - line_height_layout: "small" or "normal" or "large"
  /// - font_layout: "small", or "normal", or "large"
  pub extra: Option<String>,
}

impl View {
  pub fn new(
    view_id: ViewId,
    parent_view_id: ViewId,
    name: String,
    layout: ViewLayout,
    created_by: Option<i64>,
  ) -> Self {
    View {
      id: view_id,
      parent_view_id,
      parent_ordering: "".into(),
      name,
      created_at: 0,
      is_favorite: false,
      layout,
      icon: None,
      created_by,
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    }
  }
}

impl View {
  pub fn create_patch(&self, other: &View) -> Option<ViewPatch> {
    if self.id != other.id {
      return None;
    }

    let mut changed = false;
    let view = ViewPatch {
      id: self.id.clone(),
      timestamp: std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64,
      parent_view_id: if self.parent_view_id != other.parent_view_id {
        changed = true;
        Some(other.parent_view_id.clone())
      } else {
        None
      },
      parent_ordering: if self.parent_ordering != other.parent_ordering {
        changed = true;
        Some(other.parent_ordering.clone())
      } else {
        None
      },
      name: if self.name != other.name {
        changed = true;
        Some(other.name.clone())
      } else {
        None
      },
      is_favorite: if self.is_favorite != other.is_favorite {
        changed = true;
        Some(other.is_favorite)
      } else {
        None
      },
      layout: if self.layout != other.layout {
        changed = true;
        Some(other.layout.clone())
      } else {
        None
      },
      icon: if self.icon != other.icon {
        changed = true;
        Some(other.icon.clone())
      } else {
        None
      },
      is_locked: if self.is_locked != other.is_locked {
        changed = true;
        Some(other.is_locked)
      } else {
        None
      },
      extra: if self.extra != other.extra {
        changed = true;
        Some(other.extra.clone())
      } else {
        None
      },
    };
    if changed { Some(view) } else { None }
  }
}

pub struct ViewPatch {
  pub id: ViewId,
  pub timestamp: u64,
  pub parent_view_id: Option<ViewId>,
  pub parent_ordering: Option<FractionalIndex>,
  pub name: Option<String>,
  pub is_favorite: Option<bool>,
  pub layout: Option<ViewLayout>,
  pub icon: Option<Option<ViewIcon>>,
  pub is_locked: Option<Option<bool>>,
  pub extra: Option<Option<String>>,
}

impl ViewPatch {
  pub fn new(id: ViewId) -> Self {
    Self {
      id,
      timestamp: std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64,
      parent_view_id: None,
      parent_ordering: None,
      name: None,
      is_favorite: None,
      layout: None,
      icon: None,
      is_locked: None,
      extra: None,
    }
  }
}
