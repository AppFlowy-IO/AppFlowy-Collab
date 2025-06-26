use crate::util::{create_folder_with_workspace, make_test_view, setup_log};
use collab::core::collab::{IndexContent, default_client_id};
use collab_folder::folder_diff::FolderViewChange;
use collab_folder::{IconType, UserId, ViewIcon, ViewIndexContent, timestamp};

#[test]
fn create_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let o_view = make_test_view("v1", "w1", vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  // Insert a new view
  folder
    .body
    .views
    .insert(&mut txn, o_view.clone(), None, uid.as_i64());

  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(o_view.name, r_view.name);
  assert_eq!(o_view.parent_view_id, r_view.parent_view_id);
  assert_eq!(o_view.children, r_view.children);
}

#[test]
fn create_view_with_sub_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  let child_view = make_test_view("v1_1", "v1", vec![]);
  let view = make_test_view("v1", "w1", vec![child_view.id.clone()]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, child_view.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view.clone(), None, uid.as_i64());

  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(view.name, r_view.name);
  assert_eq!(view.parent_view_id, r_view.parent_view_id);
  assert_eq!(view.children, r_view.children);

  let r_sub_view = folder
    .body
    .views
    .get_view(&txn, &r_view.children[0].id, uid.as_i64())
    .unwrap();
  assert_eq!(child_view.name, r_sub_view.name);
  assert_eq!(child_view.parent_view_id, r_sub_view.parent_view_id);

  let views = folder.body.views.get_all_views(&txn, uid.as_i64());
  assert_eq!(views.len(), 3);
}

#[test]
fn delete_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  folder
    .body
    .views
    .insert(&mut txn, view_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_3, None, uid.as_i64());

  let views = folder
    .body
    .views
    .get_views(&txn, &["v1", "v2", "v3"], uid.as_i64());
  assert_eq!(views[0].id, "v1");
  assert_eq!(views[1].id, "v2");
  assert_eq!(views[2].id, "v3");

  folder
    .body
    .views
    .delete_views(&mut txn, vec!["v1", "v2", "v3"]);

  let views = folder
    .body
    .views
    .get_views(&txn, &["v1", "v2", "v3"], uid.as_i64());
  assert_eq!(views.len(), 0);
}

#[test]
fn update_view_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let time = timestamp();
  let o_view = make_test_view("v1", "w1", vec![]);
  folder
    .body
    .views
    .insert(&mut txn, o_view, None, uid.as_i64());
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| {
        update
          .set_name("Untitled")
          .set_desc("My first view")
          .set_favorite(true)
          .done()
      },
      uid.as_i64(),
    )
    .unwrap();

  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.name, "Untitled");
  assert!(r_view.is_favorite);
  assert_eq!(r_view.created_at, time);
  assert_eq!(r_view.last_edited_time, time);
}

#[test]
fn update_view_icon_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let o_view = make_test_view("v1", "w1", vec![]);
  folder
    .body
    .views
    .insert(&mut txn, o_view, None, uid.as_i64());

  let time = timestamp();
  let icon = ViewIcon {
    ty: IconType::Emoji,
    value: "👍".to_string(),
  };
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(Some(icon.clone())).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, Some(icon));

  let new_icon = ViewIcon {
    ty: IconType::Emoji,
    value: "👎".to_string(),
  };
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(Some(new_icon.clone())).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, Some(new_icon));
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(None).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, None);
  assert_eq!(r_view.last_edited_by, Some(uid.as_i64()));
  assert!(r_view.last_edited_time >= time);
}

#[test]
fn different_icon_ty_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let o_view = make_test_view("v1", "w1", vec![]);
  folder
    .body
    .views
    .insert(&mut txn, o_view, None, uid.as_i64());
  let emoji = ViewIcon {
    ty: IconType::Emoji,
    value: "👍".to_string(),
  };
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(Some(emoji.clone())).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, Some(emoji));

  let icon = ViewIcon {
    ty: IconType::Icon,
    value: "👍".to_string(),
  };
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(Some(icon.clone())).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, Some(icon));

  let url = ViewIcon {
    ty: IconType::Url,
    value: "https://www.notion.so/favicon.ico".to_string(),
  };
  folder
    .body
    .views
    .update_view(
      &mut txn,
      "v1",
      |update| update.set_icon(Some(url.clone())).done(),
      uid.as_i64(),
    )
    .unwrap();
  let r_view = folder
    .body
    .views
    .get_view(&txn, "v1", uid.as_i64())
    .unwrap();
  assert_eq!(r_view.icon, Some(url));
}

#[test]
fn dissociate_and_associate_view_test() {
  let uid = UserId::from(1);
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace(uid.clone(), workspace_id);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder
    .body
    .views
    .insert(&mut txn, view_1_child, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_1, None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2, None, uid.as_i64());

  let r_view = folder
    .body
    .views
    .get_view(&txn, view_1_id, uid.as_i64())
    .unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  // move out not exist parent view
  folder
    .body
    .views
    .dissociate_parent_child(&mut txn, "not_exist_parent_view", "not_exist_view");

  // move in not exist parent view
  folder.body.views.associate_parent_child(
    &mut txn,
    "not_exist_parent_view",
    "not_exist_view",
    None,
  );

  // move out view_1_child from view_2
  folder
    .body
    .views
    .dissociate_parent_child(&mut txn, view_2_id, view_1_child_id);
  let r_view = folder
    .body
    .views
    .get_view(&txn, view_2_id, uid.as_i64())
    .unwrap();
  assert_eq!(r_view.children.items.iter().len(), 0);

  folder
    .body
    .views
    .associate_parent_child(&mut txn, view_1_id, view_2_id, None);

  let r_view = folder
    .body
    .views
    .get_view(&txn, view_1_id, uid.as_i64())
    .unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.first().unwrap().id, view_2_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_1_child_id);

  folder
    .body
    .views
    .dissociate_parent_child(&mut txn, view_1_id, view_2_id);
  let r_view = folder
    .body
    .views
    .get_view(&txn, view_1_id, uid.as_i64())
    .unwrap();
  assert_eq!(r_view.children.items.iter().len(), 1);

  folder.body.views.associate_parent_child(
    &mut txn,
    view_1_id,
    view_2_id,
    Some(view_1_child_id.to_string()),
  );

  let r_view = folder
    .body
    .views
    .get_view(&txn, view_1_id, uid.as_i64())
    .unwrap();
  assert_eq!(r_view.children.items.iter().len(), 2);
  assert_eq!(r_view.children.items.first().unwrap().id, view_1_child_id);
  assert_eq!(r_view.children.items.get(1).unwrap().id, view_2_id);
}

#[test]
fn move_view_across_parent_test() {
  let uid = UserId::from(1);
  let workspace_id = "w1";
  let view_1_child_id = "v1_1";
  let view_1_id = "v1";
  let view_2_id = "v2";
  let folder_test = create_folder_with_workspace(uid.clone(), workspace_id);

  let mut folder = folder_test.folder;

  let view_1_child = make_test_view(view_1_child_id, view_1_id, vec![]);
  let view_1 = make_test_view(view_1_id, workspace_id, vec![view_1_child_id.to_string()]);
  let view_2 = make_test_view(view_2_id, workspace_id, vec![]);
  folder.insert_view(view_1_child, None, uid.as_i64());
  folder.insert_view(view_1, None, uid.as_i64());
  folder.insert_view(view_2, None, uid.as_i64());

  // Move out of the current workspace.
  let res = folder.move_nested_view(view_1_child_id, "w2", None, uid.as_i64());
  assert!(res.is_none());
  // Move view_1_child from view_1 to view_2.
  folder.move_nested_view(view_1_child_id, view_2_id, None, uid.as_i64());
  let view_1 = folder.get_view(view_1_id, uid.as_i64()).unwrap();
  let view_2 = folder.get_view(view_2_id, uid.as_i64()).unwrap();
  let view_1_child = folder.get_view(view_1_child_id, uid.as_i64()).unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 1);
  assert_eq!(view_1_child.parent_view_id, view_2_id);

  // Move view_1_child from view_2 to current workspace
  folder.move_nested_view(view_1_child_id, workspace_id, None, uid.as_i64());
  let view_1 = folder.get_view(view_1_id, uid.as_i64()).unwrap();
  let view_2 = folder.get_view(view_2_id, uid.as_i64()).unwrap();
  let view_1_child = folder.get_view(view_1_child_id, uid.as_i64()).unwrap();
  let workspace = folder
    .get_workspace_info(workspace_id, uid.as_i64())
    .unwrap();
  assert_eq!(view_1.children.items.iter().len(), 0);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(view_1_child.parent_view_id, workspace_id);
  assert_eq!(workspace.child_views.items.len(), 3);
  assert_eq!(
    workspace.child_views.items.first().unwrap().id,
    view_1_child_id
  );

  // Move view_1_child from position 0 to position 1 in the current workspace.
  folder.move_nested_view(
    view_1_child_id,
    workspace_id,
    Some(view_1_id.to_string()),
    uid.as_i64(),
  );
  let view_1 = folder.get_view(view_1_id, uid.as_i64()).unwrap();
  let view_2 = folder.get_view(view_2_id, uid.as_i64()).unwrap();
  let view_1_child = folder.get_view(view_1_child_id, uid.as_i64()).unwrap();
  let workspace = folder
    .get_workspace_info(workspace_id, uid.as_i64())
    .unwrap();
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
  folder.move_nested_view(view_1_child_id, view_1_id, None, uid.as_i64());
  let view_1 = folder.get_view(view_1_id, uid.as_i64()).unwrap();
  let view_2 = folder.get_view(view_2_id, uid.as_i64()).unwrap();
  let view_1_child = folder.get_view(view_1_child_id, uid.as_i64()).unwrap();
  let workspace = folder
    .get_workspace_info(workspace_id, uid.as_i64())
    .unwrap();
  assert_eq!(view_1.children.items.iter().len(), 1);
  assert_eq!(view_1.children.items.first().unwrap().id, view_1_child_id);
  assert_eq!(view_1_child.parent_view_id, view_1_id);
  assert_eq!(view_2.children.items.iter().len(), 0);
  assert_eq!(workspace.child_views.items.len(), 2);
}

#[test]
fn create_view_test_with_index() {
  // steps
  // 1. v1
  // 2. v2 -> v1
  // 3. v2 -> v3 -> v1
  // 4. v2 -> v3 -> v1 -> v4
  // 5. v2 -> v3 -> v1 -> v4 -> v5
  // 6. v2 -> v3 -> v1 -> v6 -> v4 -> v5
  let uid = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);
  let mut folder = folder_test.folder;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  let view_4 = make_test_view("v4", "w1", vec![]);
  let view_5 = make_test_view("v5", "w1", vec![]);
  let view_6 = make_test_view("v6", "w1", vec![]);

  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, view_1.clone(), Some(0), uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_2.clone(), Some(0), uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_3.clone(), Some(1), uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_4.clone(), Some(100), uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_5.clone(), None, uid.as_i64());
  folder
    .body
    .views
    .insert(&mut txn, view_6.clone(), Some(3), uid.as_i64());

  let views = folder
    .body
    .views
    .get_views_belong_to(&txn, &workspace_id, uid.as_i64());
  assert_eq!(views.first().unwrap().id, view_2.id);
  assert_eq!(views.get(1).unwrap().id, view_3.id);
  assert_eq!(views.get(2).unwrap().id, view_1.id);
  assert_eq!(views.get(3).unwrap().id, view_6.id);
  assert_eq!(views.get(4).unwrap().id, view_4.id);
  assert_eq!(views.get(5).unwrap().id, view_5.id);
}

#[test]
fn check_created_and_edited_time_test() {
  let uid = UserId::from(12345);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);
  let view = make_test_view("v1", "w1", vec![]);

  let mut folder = folder_test.folder;
  let mut txn = folder.collab.transact_mut();

  folder
    .body
    .views
    .insert(&mut txn, view, Some(0), uid.as_i64());
  let views = folder
    .body
    .views
    .get_views_belong_to(&txn, &workspace_id, uid.as_i64());
  let v1 = views.first().unwrap();
  assert_eq!(v1.created_by.unwrap(), uid.as_i64());
  assert_eq!(v1.last_edited_by.unwrap(), uid.as_i64());
  assert_eq!(v1.last_edited_time, v1.created_at);
}
#[tokio::test]
async fn create_view_and_then_sub_index_content_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");
  folder_test
    .folder
    .subscribe_view_change(uid.as_i64())
    .await
    .unwrap();

  let mut index_content_rx = folder_test.subscribe_index_content();
  let o_view = make_test_view("v1", "w1", vec![]);
  let mut folder = folder_test.folder;

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

  {
    let mut txn = folder.collab.transact_mut();

    // Insert a new view
    folder
      .body
      .views
      .insert(&mut txn, o_view.clone(), None, uid.as_i64());

    let r_view = folder
      .body
      .views
      .get_view(&txn, "v1", uid.as_i64())
      .unwrap();
    assert_eq!(o_view.name, r_view.name);
    assert_eq!(o_view.parent_view_id, r_view.parent_view_id);
    assert_eq!(o_view.children, r_view.children);
  }

  // check the index content
  let index_content = rx.await.unwrap();
  assert_eq!(index_content.id, o_view.id);
  assert_eq!(index_content.parent_view_id, o_view.parent_view_id);
  assert_eq!(index_content.name, o_view.name);
}

#[test]
fn compare_diff_view_test() {
  setup_log();
  let uid = UserId::from(1);
  let workspace_id = "w1".to_string();
  let folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);
  let mut folder = folder_test.folder;

  // Save the full backup of the folder
  let encode_collab = folder.encode_collab().unwrap();
  {
    let mut txn = folder.collab.transact_mut();

    // insert two views
    let view_1 = make_test_view("v1", "w1", vec![]);
    let view_2 = make_test_view("v2", "w1", vec![]);
    folder
      .body
      .views
      .insert(&mut txn, view_1, None, uid.as_i64());
    folder
      .body
      .views
      .insert(&mut txn, view_2, None, uid.as_i64());
  }

  // Calculate the changes based on the previous backup
  let changes = folder
    .calculate_view_changes(encode_collab, default_client_id())
    .unwrap();
  assert!(changes.contains(&FolderViewChange::Inserted {
    view_id: "v1".to_string(),
  }));
  assert!(changes.contains(&FolderViewChange::Inserted {
    view_id: "v2".to_string(),
  }));

  // delete v1 and then update v2
  let encode_collab = folder.encode_collab().unwrap();

  {
    let mut txn = folder.collab.transact_mut();
    folder.body.views.delete_views(&mut txn, vec!["v1"]);
    folder
      .body
      .views
      .update_view(
        &mut txn,
        "v2",
        |update| update.set_name("v2_updated").done(),
        uid.as_i64(),
      )
      .unwrap();
  }

  let changes = folder
    .calculate_view_changes(encode_collab, default_client_id())
    .unwrap();
  assert!(changes.contains(&FolderViewChange::Deleted {
    view_ids: vec!["v1".to_string()],
  }));

  assert!(changes.contains(&FolderViewChange::Updated {
    view_id: "v2".to_string(),
  }));
}
