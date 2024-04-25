use crate::util::{create_folder_with_workspace, make_test_view};
use collab::core::collab::IndexContent;
use collab_folder::{timestamp, IconType, UserId, ViewIcon, ViewIndexContent};

#[tokio::test]
async fn create_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let o_view = make_test_view("v1", "w1", vec![]);
  // Insert a new view
  folder_test.insert_view(o_view.clone(), None);

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(o_view.name, r_view.name);
  assert_eq!(o_view.parent_view_id, r_view.parent_view_id);
  assert_eq!(o_view.children, r_view.children);
}

#[tokio::test]
async fn create_view_with_sub_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let child_view = make_test_view("v1_1", "v1", vec![]);
  let view = make_test_view("v1", "w1", vec![child_view.id.clone()]);

  folder_test.insert_view(child_view.clone(), None);
  folder_test.insert_view(view.clone(), None);

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(view.name, r_view.name);
  assert_eq!(view.parent_view_id, r_view.parent_view_id);
  assert_eq!(view.children, r_view.children);

  let r_sub_view = folder_test.views.get_view(&r_view.children[0].id).unwrap();
  assert_eq!(child_view.name, r_sub_view.name);
  assert_eq!(child_view.parent_view_id, r_sub_view.parent_view_id);

  let views = folder_test.views.get_all_views();
  assert_eq!(views.len(), 3);
}

#[tokio::test]
async fn delete_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);
  folder_test.insert_view(view_3, None);

  let views = folder_test.views.get_views(&["v1", "v2", "v3"]);
  assert_eq!(views[0].id, "v1");
  assert_eq!(views[1].id, "v2");
  assert_eq!(views[2].id, "v3");

  folder_test.views.delete_views(vec!["v1", "v2", "v3"]);

  let views = folder_test.views.get_views(&["v1", "v2", "v3"]);
  assert_eq!(views.len(), 0);
}

#[tokio::test]
async fn update_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let time = timestamp();
  let o_view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(o_view, None);
  folder_test
    .views
    .update_view("v1", |update| {
      update
        .set_name("Untitled")
        .set_desc("My first view")
        .set_favorite(true)
        .done()
    })
    .unwrap();

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.name, "Untitled");
  assert_eq!(r_view.desc, "My first view");
  assert!(r_view.is_favorite);
  assert_eq!(r_view.created_at, time);
  assert_eq!(r_view.last_edited_time, time);
}

#[tokio::test]
async fn update_view_icon_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let o_view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(o_view, None);

  let time = timestamp();
  let icon = ViewIcon {
    ty: IconType::Emoji,
    value: "👍".to_string(),
  };
  folder_test
    .views
    .update_view("v1", |update| update.set_icon(Some(icon.clone())).done())
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, Some(icon));

  let new_icon = ViewIcon {
    ty: IconType::Emoji,
    value: "👎".to_string(),
  };
  folder_test
    .views
    .update_view("v1", |update| {
      update.set_icon(Some(new_icon.clone())).done()
    })
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, Some(new_icon));
  folder_test
    .views
    .update_view("v1", |update| update.set_icon(None).done())
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, None);
  assert_eq!(r_view.last_edited_by, Some(uid.as_i64()));
  assert!(r_view.last_edited_time >= time);
}

#[tokio::test]
async fn different_icon_ty_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let o_view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(o_view, None);
  let emoji = ViewIcon {
    ty: IconType::Emoji,
    value: "👍".to_string(),
  };
  folder_test
    .views
    .update_view("v1", |update| update.set_icon(Some(emoji.clone())).done())
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, Some(emoji));

  let icon = ViewIcon {
    ty: IconType::Icon,
    value: "👍".to_string(),
  };
  folder_test
    .views
    .update_view("v1", |update| update.set_icon(Some(icon.clone())).done())
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, Some(icon));

  let url = ViewIcon {
    ty: IconType::Url,
    value: "https://www.notion.so/favicon.ico".to_string(),
  };
  folder_test
    .views
    .update_view("v1", |update| update.set_icon(Some(url.clone())).done())
    .unwrap();
  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.icon, Some(url));
}

#[tokio::test]
async fn dissociate_and_associate_view_test() {
  let uid = UserId::from(1);
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace(uid.clone(), workspace_id).await;
  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder_test.insert_view(view_1_child, None);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  // move out not exist parent view
  folder_test
    .views
    .dissociate_parent_child("not_exist_parent_view", "not_exist_view");

  // move in not exist parent view
  folder_test
    .views
    .associate_parent_child("not_exist_parent_view", "not_exist_view", None);

  // move out view_1_child from view_2
  folder_test
    .views
    .dissociate_parent_child(view_2_id, view_1_child_id);
  let r_view = folder_test.views.get_view(view_2_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 0);

  folder_test
    .views
    .associate_parent_child(view_1_id, view_2_id, None);

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.first().unwrap().id, view_2_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_1_child_id);

  folder_test
    .views
    .dissociate_parent_child(view_1_id, view_2_id);
  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  folder_test
    .views
    .associate_parent_child(view_1_id, view_2_id, Some(view_1_child_id.to_string()));

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.first().unwrap().id, view_1_child_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_2_id);
}

#[tokio::test]
async fn move_view_across_parent_test() {
  let uid = UserId::from(1);
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace(uid.clone(), workspace_id).await;
  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder_test.insert_view(view_1_child, None);
  folder_test.insert_view(view_1, None);
  folder_test.insert_view(view_2, None);

  // Move out of the current workspace.
  let res = folder_test.move_nested_view(view_1_child_id, "w2", None);
  assert!(res.is_none());
  // Move view_1_child from view_1 to view_2.
  folder_test.move_nested_view(view_1_child_id, view_2_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 1);
  assert_eq!(view_1_child.parent_view_id, view_2_id);

  // Move view_1_child from view_2 to current workspace
  folder_test.move_nested_view(view_1_child_id, workspace_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_workspace_info(workspace_id).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(view_1_child.parent_view_id, workspace_id);
  assert_eq!(workspace.child_views.items.len(), 3);
  assert_eq!(
    workspace.child_views.items.first().unwrap().id,
    view_1_child_id
  );

  // Move view_1_child from position 0 to position 1 in the current workspace.
  folder_test.move_nested_view(view_1_child_id, workspace_id, Some(view_1_id.to_string()));
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_workspace_info(workspace_id).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(view_1_child.parent_view_id, workspace_id);
  assert_eq!(workspace.child_views.items.len(), 3);
  assert_eq!(
    workspace.child_views.items.get(1).unwrap().id,
    view_1_child_id
  );
  assert_eq!(workspace.child_views.items.first().unwrap().id, view_1_id);

  // move view_1_child from current workspace to view_1
  folder_test.move_nested_view(view_1_child_id, view_1_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_workspace_info(workspace_id).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 1);
  assert_eq!(view_1.children.items.first().unwrap().id, view_1_child_id);
  assert_eq!(view_1_child.parent_view_id, view_1_id);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(workspace.child_views.items.len(), 2);
}

#[tokio::test]
async fn create_view_test_with_index() {
  // steps
  // 1. v1
  // 2. v2 -> v1
  // 3. v2 -> v3 -> v1
  // 4. v2 -> v3 -> v1 -> v4
  // 5. v2 -> v3 -> v1 -> v4 -> v5
  // 6. v2 -> v3 -> v1 -> v6 -> v4 -> v5
  let uid = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id).await;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  let view_4 = make_test_view("v4", "w1", vec![]);
  let view_5 = make_test_view("v5", "w1", vec![]);
  let view_6 = make_test_view("v6", "w1", vec![]);

  folder_test.insert_view(view_1.clone(), Some(0));
  folder_test.insert_view(view_2.clone(), Some(0));
  folder_test.insert_view(view_3.clone(), Some(1));
  folder_test.insert_view(view_4.clone(), Some(100));
  folder_test.insert_view(view_5.clone(), None);
  folder_test.insert_view(view_6.clone(), Some(3));

  let views = folder_test.get_views_belong_to(&workspace_id);
  assert_eq!(views.first().unwrap().id, view_2.id);
  assert_eq!(views.get(1).unwrap().id, view_3.id);
  assert_eq!(views.get(2).unwrap().id, view_1.id);
  assert_eq!(views.get(3).unwrap().id, view_6.id);
  assert_eq!(views.get(4).unwrap().id, view_4.id);
  assert_eq!(views.get(5).unwrap().id, view_5.id);
}

#[tokio::test]
async fn check_created_and_edited_time_test() {
  let uid = UserId::from(12345);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id).await;
  let view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(view, Some(0));
  let views = folder_test.get_views_belong_to(&workspace_id);
  let v1 = views.first().unwrap();
  assert_eq!(v1.created_by.unwrap(), uid.as_i64());
  assert_eq!(v1.last_edited_by.unwrap(), uid.as_i64());
  assert_eq!(v1.last_edited_time, v1.created_at);
}
#[tokio::test]
async fn create_view_and_then_sub_index_content_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let mut index_content_rx = folder_test.subscribe_index_content();
  let o_view = make_test_view("v1", "w1", vec![]);

  // subscribe the index content
  let (tx, rx) = tokio::sync::oneshot::channel();
  tokio::spawn(async move {
    if let IndexContent::Create(json) = index_content_rx.recv().await.unwrap() {
      tx.send(serde_json::from_value::<ViewIndexContent>(json).unwrap())
        .unwrap();
    } else {
      panic!("expected IndexContent::Create");
    }
  });

  // Insert a new view
  folder_test.insert_view(o_view.clone(), None);

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(o_view.name, r_view.name);
  assert_eq!(o_view.parent_view_id, r_view.parent_view_id);
  assert_eq!(o_view.children, r_view.children);

  // check the index content
  let index_content = rx.await.unwrap();
  assert_eq!(index_content.id, o_view.id);
  assert_eq!(index_content.parent_view_id, o_view.parent_view_id);
  assert_eq!(index_content.name, o_view.name);
}
