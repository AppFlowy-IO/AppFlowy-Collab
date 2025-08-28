use crate::v2::fractional_index::{FractionalIndex, FractionalVec};
use crate::{
  FolderData, RepeatedViewIdentifier, Section, SectionItem, SectionsByUid, View, ViewIcon, ViewId,
  ViewIdentifier, ViewLayout, Workspace,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FolderState {
  pub workspace_id: ViewId,
  pub name: String,
  pub child_views: FractionalVec<ViewIdentifier>,
  pub created_at: i64,
  pub created_by: Option<i64>,
  pub last_edited_time: i64,
  pub last_edited_by: Option<i64>,
  pub current_views: HashMap<i64, ViewId>,
  pub views: HashMap<ViewId, ViewData>,
  pub sections: HashMap<Section, HashMap<i64, FractionalVec<SectionItem>>>,
}

impl FolderState {
  pub fn new(worskspace_id: String) -> FolderState {
    FolderState {
      workspace_id: worskspace_id.into(),
      name: "".to_string(),
      child_views: Default::default(),
      created_at: 0,
      created_by: None,
      last_edited_time: 0,
      last_edited_by: None,
      current_views: Default::default(),
      views: Default::default(),
      sections: Default::default(),
    }
  }

  pub fn workspace(&self) -> Workspace {
    Workspace {
      id: self.workspace_id.clone(),
      name: self.name.clone(),
      child_views: RepeatedViewIdentifier {
        items: self.child_views.iter().cloned().collect(),
      },
      created_at: self.created_at,
      created_by: self.created_by,
      last_edited_time: self.last_edited_time,
      last_edited_by: self.last_edited_by,
    }
  }
}

impl From<FolderData> for FolderState {
  fn from(value: FolderData) -> Self {
    fn fill(source: SectionsByUid, dest: &mut HashMap<i64, FractionalVec<SectionItem>>) {
      for (uid, items) in source {
        dest.insert(uid.as_i64(), FractionalVec::from_iter(items));
      }
    }

    let mut views = HashMap::new();
    for v in value.views {
      views.insert(v.id.clone(), v.into());
    }

    let sections = {
      let mut sections = HashMap::new();
      fill(value.trash, sections.entry(Section::Trash).or_default());
      fill(
        value.favorites,
        sections.entry(Section::Favorite).or_default(),
      );
      fill(value.private, sections.entry(Section::Private).or_default());
      fill(value.recent, sections.entry(Section::Recent).or_default());
      sections
    };

    FolderState {
      workspace_id: value.workspace.id.clone(),
      name: value.workspace.name.clone(),
      child_views: value.workspace.child_views.items.iter().cloned().collect(),
      created_at: value.workspace.created_at,
      created_by: value.workspace.created_by,
      last_edited_time: value.workspace.last_edited_time,
      last_edited_by: value.workspace.last_edited_by,
      current_views: HashMap::from([(value.uid, value.current_view)]),
      views,
      sections,
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ViewData {
  /// The id of the view
  pub id: ViewId,
  /// The id for given parent view
  pub parent_view_id: ViewId,
  /// A list of ids, each of them is the id of other view
  pub parent_ordering: FractionalIndex,
  /// The name that display on the left sidebar
  pub name: String,
  pub created_at: i64,
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

impl ViewData {
  pub fn new(
    view_id: ViewId,
    parent_view_id: ViewId,
    name: String,
    layout: ViewLayout,
    created_by: Option<i64>,
  ) -> Self {
    ViewData {
      id: view_id,
      parent_view_id,
      parent_ordering: "".into(),
      name,
      created_at: 0,
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

impl From<ViewData> for View {
  fn from(value: ViewData) -> Self {
    View {
      id: value.id,
      parent_view_id: value.parent_view_id,
      name: value.name,
      created_at: value.created_at,
      layout: value.layout,
      icon: value.icon,
      created_by: value.created_by,
      last_edited_time: value.last_edited_time,
      last_edited_by: value.last_edited_by,
      is_locked: value.is_locked,
      extra: value.extra,

      // these need to be populated separately
      children: Default::default(),
      is_favorite: false,
    }
  }
}

impl From<View> for ViewData {
  fn from(value: View) -> Self {
    ViewData {
      id: value.id,
      parent_view_id: value.parent_view_id,
      parent_ordering: "".into(), // needs to be set separately
      name: value.name,
      created_at: value.created_at,
      layout: value.layout,
      icon: value.icon,
      created_by: value.created_by,
      last_edited_time: value.last_edited_time,
      last_edited_by: value.last_edited_by,
      is_locked: value.is_locked,
      extra: value.extra,
    }
  }
}

impl ViewData {
  pub fn create_patch(&self, other: &ViewData) -> Option<ViewPatch> {
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
      layout: None,
      icon: None,
      is_locked: None,
      extra: None,
    }
  }
}
