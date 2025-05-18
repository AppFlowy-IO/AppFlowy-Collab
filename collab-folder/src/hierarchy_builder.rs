use crate::space_info::SpacePermission;
use crate::{
  IconType, RepeatedViewIdentifier, SPACE_CREATED_AT_KEY, SPACE_ICON_COLOR_KEY, SPACE_ICON_KEY,
  SPACE_IS_SPACE_KEY, SPACE_PERMISSION_KEY, SpaceInfo, View, ViewIcon, ViewIdentifier, ViewLayout,
  timestamp,
};

use serde_json::json;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::ops::{Deref, DerefMut};

/// A builder for creating a view for a workspace.
/// The views created by this builder will be the first level views of the workspace.
pub struct NestedViewBuilder {
  pub uid: i64,
  pub workspace_id: String,
  pub views: Vec<ParentChildViews>,
}

impl NestedViewBuilder {
  pub fn new(workspace_id: String, uid: i64) -> Self {
    Self {
      uid,
      workspace_id,
      views: vec![],
    }
  }

  pub async fn with_view_builder<F, O>(&mut self, view_builder: F) -> &mut Self
  where
    F: Fn(NestedChildViewBuilder) -> O,
    O: Future<Output = ParentChildViews>,
  {
    let builder = NestedChildViewBuilder::new(self.uid, self.workspace_id.clone());
    let view = view_builder(builder).await;
    self.views.push(view);
    self
  }

  pub fn build(&mut self) -> NestedViews {
    NestedViews {
      views: std::mem::take(&mut self.views),
    }
  }
}

#[derive(Debug, Clone)]
pub struct NestedViews {
  pub views: Vec<ParentChildViews>,
}

impl Display for NestedViews {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for view in &self.views {
      write!(f, "{}", view)?;
    }
    Ok(())
  }
}

impl NestedViews {
  pub fn into_inner(self) -> Vec<ParentChildViews> {
    self.views
  }

  pub fn remove_view(&mut self, view_id: &str) {
    // recursively remove the view and its children views.
    self.views.retain_mut(|view| {
      if view.view.id == view_id {
        return false;
      }
      view.remove_view(view_id);
      true
    });
  }

  pub fn find_view(&self, view_id: &str) -> Option<&View> {
    for view in &self.views {
      let view = view.find_view(view_id);
      if view.is_some() {
        return view;
      }
    }
    None
  }

  pub fn flatten_views(&self) -> Vec<View> {
    FlattedViews::flatten_views(self.views.clone())
  }
}

impl Deref for NestedViews {
  type Target = Vec<ParentChildViews>;

  fn deref(&self) -> &Self::Target {
    &self.views
  }
}

impl DerefMut for NestedViews {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.views
  }
}

/// A builder for creating a view.
/// The default layout of the view is [ViewLayout::Document]
pub struct NestedChildViewBuilder {
  uid: i64,
  parent_view_id: String,
  view_id: String,
  name: String,
  desc: String,
  layout: ViewLayout,
  children: Vec<ParentChildViews>,
  is_favorite: bool,
  icon: Option<ViewIcon>,
  is_locked: Option<bool>,
  extra: Option<String>,
}

impl NestedChildViewBuilder {
  /// Create a new view builder.
  /// It generates a new view id for the view. If you want to specify the view id, you can use [with_view_id] method.
  pub fn new(uid: i64, parent_view_id: String) -> Self {
    Self {
      uid,
      parent_view_id,
      view_id: uuid::Uuid::new_v4().to_string(),
      name: Default::default(),
      desc: Default::default(),
      layout: ViewLayout::Document,
      children: vec![],
      is_favorite: false,
      icon: None,
      is_locked: None,
      extra: None,
    }
  }

  pub fn view_id(&self) -> &str {
    &self.view_id
  }

  pub fn with_view(mut self, view: ParentChildViews) -> Self {
    self.children.push(view);
    self
  }

  pub fn with_children(mut self, mut views: Vec<ParentChildViews>) -> Self {
    self.children.append(&mut views);
    self
  }

  pub fn with_view_id<T: ToString>(mut self, view_id: T) -> Self {
    self.view_id = view_id.to_string();
    self
  }

  pub fn with_layout(mut self, layout: ViewLayout) -> Self {
    self.layout = layout;
    self
  }

  pub fn with_name(mut self, name: &str) -> Self {
    self.name = name.to_string();
    self
  }

  pub fn with_desc(mut self, desc: &str) -> Self {
    self.desc = desc.to_string();
    self
  }

  pub fn with_icon(mut self, icon: &str) -> Self {
    self.icon = Some(ViewIcon {
      ty: IconType::Emoji,
      value: icon.to_string(),
    });
    self
  }

  pub fn with_extra<F: FnOnce(ViewExtraBuilder) -> serde_json::Value>(mut self, extra: F) -> Self {
    let builder = ViewExtraBuilder::new();
    let extra_json = extra(builder);
    self.extra = Some(serde_json::to_string(&extra_json).unwrap());
    self
  }

  /// Create a child view for the current view.
  /// The view created by this builder will be the next level view of the current view.
  pub async fn with_child_view_builder<F, O>(mut self, child_view_builder: F) -> Self
  where
    F: Fn(NestedChildViewBuilder) -> O,
    O: Future<Output = ParentChildViews>,
  {
    let builder = NestedChildViewBuilder::new(self.uid, self.view_id.clone());
    self.children.push(child_view_builder(builder).await);
    self
  }

  pub fn build(self) -> ParentChildViews {
    let view = View {
      id: self.view_id,
      parent_view_id: self.parent_view_id,
      name: self.name,
      created_at: timestamp(),
      is_favorite: self.is_favorite,
      layout: self.layout,
      icon: self.icon,
      created_by: Some(self.uid),
      last_edited_time: 0,
      children: RepeatedViewIdentifier::new(
        self
          .children
          .iter()
          .map(|v| ViewIdentifier {
            id: v.view.id.clone(),
          })
          .collect(),
      ),
      last_edited_by: Some(self.uid),
      is_locked: self.is_locked,
      extra: self.extra,
    };
    ParentChildViews {
      view,
      children: self.children,
    }
  }
}

pub struct ViewExtraBuilder(serde_json::Value);
impl Default for ViewExtraBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl ViewExtraBuilder {
  pub fn new() -> Self {
    Self(json!({}))
  }

  pub fn is_space(mut self, is_space: bool) -> Self {
    self.0[SPACE_IS_SPACE_KEY] = json!(is_space);
    if is_space {
      self.0[SPACE_CREATED_AT_KEY] = json!(timestamp());
    }
    self
  }

  pub fn with_space_icon(mut self, icon: Option<&str>) -> Self {
    if let Some(icon) = icon {
      self.0[SPACE_ICON_KEY] = json!(icon);
    }
    self
  }

  pub fn with_space_icon_color(mut self, icon_color: Option<&str>) -> Self {
    if let Some(icon_color) = icon_color {
      self.0[SPACE_ICON_COLOR_KEY] = json!(icon_color);
    }
    self
  }

  pub fn with_space_permission(mut self, permission: SpacePermission) -> Self {
    self.0[SPACE_PERMISSION_KEY] = json!(permission as u8);
    self
  }

  pub fn with_space_info(mut self, space_info: SpaceInfo) -> Self {
    self.0[SPACE_IS_SPACE_KEY] = json!(space_info.is_space);
    self.0[SPACE_PERMISSION_KEY] = json!(space_info.space_permission as u8);
    if let Some(icon) = space_info.space_icon {
      self.0[SPACE_ICON_KEY] = json!(icon);
    }
    if let Some(icon_color) = space_info.space_icon_color {
      self.0[SPACE_ICON_COLOR_KEY] = json!(icon_color);
    }
    self.0[SPACE_CREATED_AT_KEY] = json!(space_info.space_created_at);
    self
  }

  pub fn build(self) -> serde_json::Value {
    self.0
  }
}

#[derive(Debug, Clone)]
pub struct ParentChildViews {
  pub view: View,
  pub children: Vec<ParentChildViews>,
}

impl ParentChildViews {
  fn fmt_with_indent(&self, f: &mut Formatter<'_>, indent_level: usize) -> std::fmt::Result {
    let indent = "  ".repeat(indent_level);
    writeln!(
      f,
      "{}: {}, parent id: {}, layout: {:?}",
      indent, self.view.name, self.view.parent_view_id, self.view.layout,
    )?;

    // Recursively print child views
    for child in &self.children {
      child.fmt_with_indent(f, indent_level + 1)?;
    }

    Ok(())
  }
}

impl Display for ParentChildViews {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.fmt_with_indent(f, 0)
  }
}

impl ParentChildViews {
  pub fn remove_view(&mut self, view_id: &str) {
    self.children.retain_mut(|child_view| {
      if child_view.view.id == view_id {
        return false;
      }
      child_view.remove_view(view_id);
      true
    });
  }

  pub fn find_view(&self, view_id: &str) -> Option<&View> {
    if self.view.id == view_id {
      return Some(&self.view);
    }
    for child_view in &self.children {
      let view = child_view.find_view(view_id);
      if view.is_some() {
        return view;
      }
    }
    None
  }
}

pub struct FlattedViews;

impl FlattedViews {
  pub fn flatten_views(views: Vec<ParentChildViews>) -> Vec<View> {
    let mut result = vec![];
    for view in views {
      result.push(view.view);
      result.append(&mut Self::flatten_views(view.children));
    }
    result
  }
}

#[cfg(test)]
mod tests {
  use crate::hierarchy_builder::{FlattedViews, NestedViewBuilder};

  #[tokio::test]
  async fn create_first_level_views_test() {
    let workspace_id = "w1".to_string();
    let mut builder = NestedViewBuilder::new(workspace_id, 1);
    builder
      .with_view_builder(|view_builder| async { view_builder.with_name("1").build() })
      .await;
    builder
      .with_view_builder(|view_builder| async { view_builder.with_name("2").build() })
      .await;
    builder
      .with_view_builder(|view_builder| async { view_builder.with_name("3").build() })
      .await;
    let workspace_views = builder.build();
    assert_eq!(workspace_views.len(), 3);

    let views = FlattedViews::flatten_views(workspace_views.into_inner());
    assert_eq!(views.len(), 3);
  }

  #[tokio::test]
  async fn create_view_with_children_test() {
    let workspace_id = "w1".to_string();
    let mut builder = NestedViewBuilder::new(workspace_id, 1);
    builder
      .with_view_builder(|view_builder| async {
        view_builder
          .with_name("1")
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder.with_name("1_1").build()
          })
          .await
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder.with_name("1_2").build()
          })
          .await
          .build()
      })
      .await;
    builder
      .with_view_builder(|view_builder| async {
        view_builder
          .with_name("2")
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder.with_name("2_1").build()
          })
          .await
          .build()
      })
      .await;
    let workspace_views = builder.build();
    assert_eq!(workspace_views.len(), 2);

    assert_eq!(workspace_views[0].view.name, "1");
    assert_eq!(workspace_views[0].children.len(), 2);
    assert_eq!(workspace_views[0].children[0].view.name, "1_1");
    assert_eq!(workspace_views[0].children[1].view.name, "1_2");
    assert_eq!(workspace_views[1].children.len(), 1);
    assert_eq!(workspace_views[1].children[0].view.name, "2_1");
    let views = FlattedViews::flatten_views(workspace_views.clone().into_inner());
    assert_eq!(views.len(), 5);

    {
      let mut cloned_workspace_views = workspace_views.clone();
      let view_id_1_2 = workspace_views[0].children[1].view.id.clone();
      cloned_workspace_views.remove_view(&view_id_1_2);
      let views = FlattedViews::flatten_views(cloned_workspace_views.into_inner());
      assert_eq!(views.len(), 4);
    }

    {
      let mut cloned_workspace_views = workspace_views.clone();
      let view_id_1 = workspace_views[0].view.id.clone();
      cloned_workspace_views.remove_view(&view_id_1);
      let views = FlattedViews::flatten_views(cloned_workspace_views.into_inner());
      assert_eq!(views.len(), 2);
    }
  }

  #[tokio::test]
  async fn create_three_level_view_test() {
    let workspace_id = "w1".to_string();
    let mut builder = NestedViewBuilder::new(workspace_id, 1);
    builder
      .with_view_builder(|view_builder| async {
        view_builder
          .with_name("1")
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder
              .with_name("1_1")
              .with_child_view_builder(|b| async { b.with_name("1_1_1").build() })
              .await
              .with_child_view_builder(|b| async { b.with_name("1_1_2").build() })
              .await
              .build()
          })
          .await
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder
              .with_name("1_2")
              .with_child_view_builder(|b| async { b.with_name("1_2_1").build() })
              .await
              .with_child_view_builder(|b| async { b.with_name("1_2_2").build() })
              .await
              .build()
          })
          .await
          .build()
      })
      .await;
    let workspace_views = builder.build();
    assert_eq!(workspace_views.len(), 1);

    assert_eq!(workspace_views[0].view.name, "1");
    assert_eq!(workspace_views[0].children.len(), 2);
    assert_eq!(workspace_views[0].children[0].view.name, "1_1");
    assert_eq!(workspace_views[0].children[1].view.name, "1_2");

    assert_eq!(
      workspace_views[0].children[0].children[0].view.name,
      "1_1_1"
    );
    assert_eq!(
      workspace_views[0].children[0].children[1].view.name,
      "1_1_2"
    );

    assert_eq!(
      workspace_views[0].children[1].children[0].view.name,
      "1_2_1"
    );
    assert_eq!(
      workspace_views[0].children[1].children[1].view.name,
      "1_2_2"
    );

    let views = FlattedViews::flatten_views(workspace_views.clone().into_inner());
    assert_eq!(views.len(), 7);

    {
      let mut cloned_workspace_views = workspace_views.clone();
      let view_id_1_1 = workspace_views[0].children[0].view.id.clone();
      let view_id_1_2 = workspace_views[0].children[1].view.id.clone();
      cloned_workspace_views.remove_view(&view_id_1_1);
      let views = FlattedViews::flatten_views(cloned_workspace_views.clone().into_inner());
      assert_eq!(views.len(), 4);

      cloned_workspace_views.remove_view(&view_id_1_2);
      let views = FlattedViews::flatten_views(cloned_workspace_views.into_inner());
      assert_eq!(views.len(), 1);
    }
  }

  #[tokio::test]
  async fn delete_multiple_views_in_sequence_test() {
    let workspace_id = "w1".to_string();
    let mut builder = NestedViewBuilder::new(workspace_id, 1);

    // Create a 3-level nested view hierarchy
    builder
      .with_view_builder(|view_builder| async {
        view_builder
          .with_name("Root")
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder
              .with_name("Child-1")
              .with_child_view_builder(|grandchild_view_builder| async {
                grandchild_view_builder
                  .with_name("Grandchild-1-1")
                  .with_child_view_builder(|great_grandchild_view_builder| async {
                    great_grandchild_view_builder
                      .with_name("Great-Grandchild-1-1-1")
                      .build()
                  })
                  .await
                  .build()
              })
              .await
              .with_child_view_builder(|grandchild_view_builder| async {
                grandchild_view_builder.with_name("Grandchild-1-2").build()
              })
              .await
              .build()
          })
          .await
          .with_child_view_builder(|child_view_builder| async {
            child_view_builder.with_name("Child-2").build()
          })
          .await
          .build()
      })
      .await;

    let workspace_views = builder.build();
    assert_eq!(workspace_views.len(), 1); // Ensure there is one root view

    let views = FlattedViews::flatten_views(workspace_views.clone().into_inner());
    assert_eq!(views.len(), 6); // Ensure there are 6 total views (1 root, 2 children, 2 grandchildren, 1 great-grandchild)

    // Test deleting multiple views in sequence
    {
      let mut cloned_workspace_views = workspace_views.clone();

      // First, delete a third-level view (Grandchild-1-2)
      let view_id_grandchild_1_2 = workspace_views[0].children[0].children[1].view.id.clone(); // "Grandchild-1-2"
      cloned_workspace_views.remove_view(&view_id_grandchild_1_2);
      let views_after_delete =
        FlattedViews::flatten_views(cloned_workspace_views.clone().into_inner());
      assert_eq!(views_after_delete.len(), 5); // Should have 5 views left
      assert!(
        !views_after_delete
          .iter()
          .any(|v| v.name == "Grandchild-1-2")
      );

      // Second, delete the great-grandchild (Great-Grandchild-1-1-1)
      let view_id_great_grandchild_1_1_1 = workspace_views[0].children[0].children[0].children[0]
        .view
        .id
        .clone(); // "Great-Grandchild-1-1-1"
      cloned_workspace_views.remove_view(&view_id_great_grandchild_1_1_1);
      let views_after_delete =
        FlattedViews::flatten_views(cloned_workspace_views.clone().into_inner());
      assert_eq!(views_after_delete.len(), 4); // Should have 4 views left
      assert!(
        !views_after_delete
          .iter()
          .any(|v| v.name == "Great-Grandchild-1-1-1")
      );

      // Third, delete a second-level view (Child-2)
      let view_id_child_2 = workspace_views[0].children[1].view.id.clone(); // "Child-2"
      cloned_workspace_views.remove_view(&view_id_child_2);
      let views_after_delete =
        FlattedViews::flatten_views(cloned_workspace_views.clone().into_inner());
      assert_eq!(views_after_delete.len(), 3); // Should have 3 views left
      assert!(!views_after_delete.iter().any(|v| v.name == "Child-2"));

      // Fourth, delete the root view (Root)
      let view_id_root = workspace_views[0].view.id.clone(); // "Root"
      cloned_workspace_views.remove_view(&view_id_root);
      let views_after_delete = FlattedViews::flatten_views(cloned_workspace_views.into_inner());
      assert_eq!(views_after_delete.len(), 0); // Should have no views left
    }
  }
}
