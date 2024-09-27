use crate::{
  timestamp, IconType, RepeatedViewIdentifier, View, ViewIcon, ViewIdentifier, ViewLayout,
};
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
    F: Fn(ViewBuilder) -> O,
    O: Future<Output = ParentChildViews>,
  {
    let builder = ViewBuilder::new(self.uid, self.workspace_id.clone());
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
  views: Vec<ParentChildViews>,
}

impl NestedViews {
  pub fn into_inner(self) -> Vec<ParentChildViews> {
    self.views
  }

  pub fn remove_view(&mut self, view_id: &str) {
    self.views.retain(|view| view.parent_view.id != view_id);
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
pub struct ViewBuilder {
  uid: i64,
  parent_view_id: String,
  view_id: String,
  name: String,
  desc: String,
  layout: ViewLayout,
  child_views: Vec<ParentChildViews>,
  is_favorite: bool,
  icon: Option<ViewIcon>,
  extra: Option<String>,
}

impl ViewBuilder {
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
      child_views: vec![],
      is_favorite: false,
      icon: None,
      extra: None,
    }
  }

  pub fn view_id(&self) -> &str {
    &self.view_id
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

  pub fn with_extra(mut self, extra: &str) -> Self {
    self.extra = Some(extra.to_string());
    self
  }

  /// Create a child view for the current view.
  /// The view created by this builder will be the next level view of the current view.
  pub async fn with_child_view_builder<F, O>(mut self, child_view_builder: F) -> Self
  where
    F: Fn(ViewBuilder) -> O,
    O: Future<Output = ParentChildViews>,
  {
    let builder = ViewBuilder::new(self.uid, self.view_id.clone());
    self.child_views.push(child_view_builder(builder).await);
    self
  }

  pub fn build(self) -> ParentChildViews {
    let view = View {
      id: self.view_id,
      parent_view_id: self.parent_view_id,
      name: self.name,
      desc: self.desc,
      created_at: timestamp(),
      is_favorite: self.is_favorite,
      layout: self.layout,
      icon: self.icon,
      created_by: Some(self.uid),
      last_edited_time: 0,
      children: RepeatedViewIdentifier::new(
        self
          .child_views
          .iter()
          .map(|v| ViewIdentifier {
            id: v.parent_view.id.clone(),
          })
          .collect(),
      ),
      last_edited_by: Some(self.uid),
      extra: self.extra,
    };
    ParentChildViews {
      parent_view: view,
      child_views: self.child_views,
    }
  }
}

#[derive(Debug, Clone)]
pub struct ParentChildViews {
  pub parent_view: View,
  pub child_views: Vec<ParentChildViews>,
}

pub struct FlattedViews;

impl FlattedViews {
  pub fn flatten_views(views: Vec<ParentChildViews>) -> Vec<View> {
    let mut result = vec![];
    for view in views {
      result.push(view.parent_view);
      result.append(&mut Self::flatten_views(view.child_views));
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
  async fn create_view_with_child_views_test() {
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
    let mut workspace_views = builder.build();
    assert_eq!(workspace_views.len(), 2);

    assert_eq!(workspace_views[0].parent_view.name, "1");
    assert_eq!(workspace_views[0].child_views.len(), 2);
    assert_eq!(workspace_views[0].child_views[0].parent_view.name, "1_1");
    assert_eq!(workspace_views[0].child_views[1].parent_view.name, "1_2");
    assert_eq!(workspace_views[1].child_views.len(), 1);
    assert_eq!(workspace_views[1].child_views[0].parent_view.name, "2_1");
    let views = FlattedViews::flatten_views(workspace_views.clone().into_inner());
    assert_eq!(views.len(), 5);

    {
      let mut cloned_workspace_views = workspace_views.clone();
      let view_id_1_2 = workspace_views[0].child_views[1].parent_view.id.clone();
      cloned_workspace_views.remove_view(&view_id_1_2);
      let views = FlattedViews::flatten_views(cloned_workspace_views.into_inner());
      assert_eq!(views.len(), 4);
    }

    {
      let mut cloned_workspace_views = workspace_views.clone();
      let view_id_1 = workspace_views[0].parent_view.id.clone();
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

    assert_eq!(workspace_views[0].parent_view.name, "1");
    assert_eq!(workspace_views[0].child_views.len(), 2);
    assert_eq!(workspace_views[0].child_views[0].parent_view.name, "1_1");
    assert_eq!(workspace_views[0].child_views[1].parent_view.name, "1_2");

    assert_eq!(
      workspace_views[0].child_views[0].child_views[0]
        .parent_view
        .name,
      "1_1_1"
    );
    assert_eq!(
      workspace_views[0].child_views[0].child_views[1]
        .parent_view
        .name,
      "1_1_2"
    );

    assert_eq!(
      workspace_views[0].child_views[1].child_views[0]
        .parent_view
        .name,
      "1_2_1"
    );
    assert_eq!(
      workspace_views[0].child_views[1].child_views[1]
        .parent_view
        .name,
      "1_2_2"
    );

    let views = FlattedViews::flatten_views(workspace_views.into_inner());
    assert_eq!(views.len(), 7);
  }
}
