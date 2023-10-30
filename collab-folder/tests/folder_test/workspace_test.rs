use collab_folder::{RepeatedViewIdentifier, UserId, ViewIdentifier, Workspace};

use crate::util::create_folder;

#[tokio::test]
async fn create_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;

  let child_views = RepeatedViewIdentifier {
    items: vec![
      ViewIdentifier::new("1".to_string()),
      ViewIdentifier::new("2".to_string()),
    ],
  };
  let o_workspace = Workspace {
    id: "1".to_string(),
    name: "My first workspace".to_string(),
    child_views,
    created_at: 123,
  };

  folder_test.workspaces.create_workspace(o_workspace.clone());
  let r_workspace = folder_test.workspaces.get_all_workspaces().remove(0);

  assert_eq!(o_workspace.name, r_workspace.name);
  assert_eq!(o_workspace.id, r_workspace.id);
  assert_eq!(o_workspace.child_views, r_workspace.child_views);
}

#[tokio::test]
async fn set_current_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let workspace = Workspace {
    id: "1".to_string(),
    name: "My first workspace".to_string(),
    child_views: Default::default(),
    created_at: 123,
  };

  folder_test.workspaces.create_workspace(workspace.clone());
  folder_test.set_current_workspace(&workspace.id);
  assert_eq!(
    folder_test.get_current_workspace().unwrap().id,
    workspace.id
  );

  folder_test.set_current_workspace("12345678");
  assert_eq!(
    folder_test.get_current_workspace().unwrap().id,
    workspace.id
  );
}

#[tokio::test]
async fn update_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let workspace = Workspace {
    id: "1".to_string(),
    name: "My first workspace".to_string(),
    child_views: RepeatedViewIdentifier {
      items: vec![
        ViewIdentifier::new("1".to_string()),
        ViewIdentifier::new("2".to_string()),
      ],
    },
    created_at: 123,
  };

  folder_test.workspaces.create_workspace(workspace);
  folder_test
    .workspaces
    .update_workspace("1", |workspace_update| {
      workspace_update.set_name("New workspace").delete_child(0);
    });

  // folder_test.workspaces.
  let workspace = folder_test.workspaces.get_workspace("1").unwrap();
  assert_eq!(workspace.name, "New workspace");
  assert_eq!(workspace.child_views.len(), 1);
  assert_eq!(workspace.child_views[0].id, "2");
}

#[tokio::test]
async fn get_all_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  for i in 0..5 {
    let mut child_views = vec![];
    for j in 0..i {
      child_views.push(ViewIdentifier::new(j.to_string()))
    }

    let workspace = Workspace {
      id: i.to_string(),
      name: format!("My {} workspace", i),
      child_views: RepeatedViewIdentifier { items: child_views },
      created_at: 123,
    };
    folder_test.workspaces.create_workspace(workspace);
  }

  let workspaces = folder_test.workspaces.get_all_workspaces();

  assert_eq!(workspaces.len(), 5);
  assert_eq!(workspaces[0].id, "0");
  assert_eq!(workspaces[1].id, "1");
  assert_eq!(workspaces[2].id, "2");
  assert_eq!(workspaces[3].id, "3");
  assert_eq!(workspaces[4].id, "4");

  assert_eq!(workspaces[0].child_views.len(), 0);
  assert_eq!(workspaces[1].child_views.len(), 1);
  assert_eq!(workspaces[2].child_views.len(), 2);
  assert_eq!(workspaces[3].child_views.len(), 3);
  assert_eq!(workspaces[4].child_views.len(), 4);
}

#[tokio::test]
async fn delete_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  for i in 0..10 {
    let workspace = Workspace {
      id: i.to_string(),
      name: format!("My {} workspace", i),
      child_views: Default::default(),
      created_at: 123,
    };
    folder_test.workspaces.create_workspace(workspace);
  }

  folder_test.workspaces.delete_workspace(0);
  let workspaces = folder_test.workspaces.get_all_workspaces();
  assert_eq!(workspaces.len(), 9);
  assert_eq!(workspaces[0].id, "1");
}
