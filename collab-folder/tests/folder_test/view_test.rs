use crate::util::{create_folder_with_workspace, make_test_view};

#[tokio::test]
async fn create_view_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let o_view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(o_view.clone());

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(o_view.name, r_view.name);
  assert_eq!(o_view.parent_view_id, r_view.parent_view_id);
  assert_eq!(o_view.children, r_view.children);
}

#[tokio::test]
async fn create_view_with_sub_view_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let child_view = make_test_view("v1_1", "v1", vec![]);
  let view = make_test_view("v1", "w1", vec![child_view.id.clone()]);

  folder_test.insert_view(child_view.clone());
  folder_test.insert_view(view.clone());

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(view.name, r_view.name);
  assert_eq!(view.parent_view_id, r_view.parent_view_id);
  assert_eq!(view.children, r_view.children);

  let r_sub_view = folder_test.views.get_view(&r_view.children[0].id).unwrap();
  assert_eq!(child_view.name, r_sub_view.name);
  assert_eq!(child_view.parent_view_id, r_sub_view.parent_view_id);
}

#[tokio::test]
async fn delete_view_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  folder_test.insert_view(view_1);
  folder_test.insert_view(view_2);
  folder_test.insert_view(view_3);

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
  let folder_test = create_folder_with_workspace("1", "w1");
  let o_view = make_test_view("v1", "w1", vec![]);
  folder_test.insert_view(o_view);
  folder_test
    .views
    .update_view("v1", |update| {
      update.set_name("Untitled").set_desc("My first view").done()
    })
    .unwrap();

  let r_view = folder_test.views.get_view("v1").unwrap();
  assert_eq!(r_view.name, "Untitled");
  assert_eq!(r_view.desc, "My first view");
}

#[tokio::test]
async fn move_view_out_and_move_view_in_test() {
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace("1", workspace_id);
  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder_test.insert_view(view_1_child);
  folder_test.insert_view(view_1);
  folder_test.insert_view(view_2);

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  // move out not exist parent view
  folder_test
    .views
    .move_child_out("not_exist_parent_view", "not_exist_view");

  // move in not exist parent view
  folder_test
    .views
    .move_child_in("not_exist_parent_view", "not_exist_view", None);

  // move out view_1_child from view_2
  folder_test.views.move_child_out(view_2_id, view_1_child_id);
  let r_view = folder_test.views.get_view(view_2_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 0);

  folder_test.views.move_child_in(view_1_id, view_2_id, None);

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.get(0).unwrap().id, view_2_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_1_child_id);

  folder_test.views.move_child_out(view_1_id, view_2_id);
  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  folder_test
    .views
    .move_child_in(view_1_id, view_2_id, Some(view_1_child_id.to_string()));

  let r_view = folder_test.views.get_view(view_1_id).unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.get(0).unwrap().id, view_1_child_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_2_id);
}

#[tokio::test]
async fn move_view() {
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace("1", workspace_id);
  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder_test.insert_view(view_1_child);
  folder_test.insert_view(view_1);
  folder_test.insert_view(view_2);

  // move out current workspace
  let res = folder_test.move_nested_view(view_1_child_id, "w2", None);
  assert!(res.is_none());
  // move view_1_child from view_1 to view_2
  folder_test.move_nested_view(view_1_child_id, view_2_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 1);
  assert_eq!(view_1_child.parent_view_id, view_2_id);

  // move view_1_child from view_2 to current workspace
  folder_test.move_nested_view(view_1_child_id, workspace_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_current_workspace().unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(view_1_child.parent_view_id, workspace_id);
  assert_eq!(workspace.child_views.items.len(), 3);
  assert_eq!(
    workspace.child_views.items.get(0).unwrap().id,
    view_1_child_id
  );

  // move view_1_child from 0 to 1 in current workspace
  folder_test.move_nested_view(view_1_child_id, workspace_id, Some(view_1_id.to_string()));
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_current_workspace().unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(view_1_child.parent_view_id, workspace_id);
  assert_eq!(workspace.child_views.items.len(), 3);
  assert_eq!(
    workspace.child_views.items.get(1).unwrap().id,
    view_1_child_id
  );
  assert_eq!(workspace.child_views.items.get(0).unwrap().id, view_1_id);

  // move view_1_child from current workspace to view_1
  folder_test.move_nested_view(view_1_child_id, view_1_id, None);
  let view_1 = folder_test.views.get_view(view_1_id).unwrap();
  let view_2 = folder_test.views.get_view(view_2_id).unwrap();
  let view_1_child = folder_test.views.get_view(view_1_child_id).unwrap();
  let workspace = folder_test.get_current_workspace().unwrap();
  assert_eq!(view_1.children.items.iter().len(), 1);
  assert_eq!(view_1.children.items.get(0).unwrap().id, view_1_child_id);
  assert_eq!(view_1_child.parent_view_id, view_1_id);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(workspace.child_views.items.len(), 2);
}
