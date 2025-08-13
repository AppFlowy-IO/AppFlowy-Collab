use crate::util::{FolderTest, create_folder, make_test_view};
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{Collab, Update};
use collab_folder::{Folder, FolderData, UserId};

#[test]
fn replace_view_get_view() {
  let uid = 1i64;
  let mut folder = create_folder(UserId::from(uid), "fake_workspace_id");
  let workspace_id = folder.get_workspace_id().unwrap();

  // Create initial views
  let v1 = make_test_view("v1", &workspace_id, vec![]);
  let v21 = make_test_view("v2.1", &workspace_id, vec![]);
  let v22 = make_test_view("v2.2", &workspace_id, vec![]);
  let v2 = make_test_view("v2", &workspace_id, vec!["v2.1".to_string(), "v2.2".into()]);
  folder.insert_view(v1, None, uid);
  folder.insert_view(v21, None, uid);
  folder.insert_view(v22, None, uid);
  folder.insert_view(v2, None, uid);

  let old = folder.get_view("v2", uid).unwrap();
  assert_eq!(old.id, "v2");

  folder.replace_view("v2", "v3", uid);

  // getting old view id should return new one
  let new = folder.get_view("v2", uid).unwrap();
  assert_eq!(new.id, "v3");
  assert_eq!(new.name, old.name);
  assert_eq!(new.parent_view_id, old.parent_view_id);
  assert_eq!(new.children, old.children);
  assert_eq!(new.layout, old.layout);
  assert_eq!(new.icon, old.icon);
}

#[test]
fn replace_view_get_view_concurrent_update() {
  let uid1 = 1i64;
  let uid2 = 1i64;
  let mut f1 = create_folder(UserId::from(uid1), "fake_workspace_id");
  let workspace_id = f1.get_workspace_id().unwrap();

  // Create initial views
  let v1 = make_test_view("v1", &workspace_id, vec![]);
  let v21 = make_test_view("v2.1", &workspace_id, vec![]);
  let v22 = make_test_view("v2.2", &workspace_id, vec![]);
  let v2 = make_test_view("v2", &workspace_id, vec!["v2.1".to_string(), "v2.2".into()]);
  f1.insert_view(v1, None, uid1);
  f1.insert_view(v21, None, uid1);
  f1.insert_view(v22, None, uid1);
  f1.insert_view(v2, None, uid1);

  let mut collab = Collab::new(uid2, "fake_workspace_id", "device-2", 2);

  // sync initial state between f1 and f2
  collab
    .apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  let mut f2 = Folder::open(collab, None).unwrap();

  assert_eq!(f1.to_json_value(), f2.to_json_value());

  // concurrently replace view in f1 and add new child view in f2
  f1.replace_view("v2", "v3", uid1);

  let mut v23 = make_test_view("v2.3", &workspace_id, vec![]);
  v23.parent_view_id = "v2".to_string();
  f2.insert_view(v23, None, uid2);

  // cross-sync state between f1 and f2
  f2.apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  f1.apply_update(Update::decode_v2(&f2.encode_collab_v2().doc_state).unwrap())
    .unwrap();

  let view = f1.get_view("v2", uid2).unwrap();
  assert_eq!(view.id, "v3");
  assert_eq!(
    view
      .children
      .iter()
      .map(|c| c.id.clone())
      .collect::<Vec<_>>(),
    vec!["v2.1", "v2.2", "v2.3"]
  );

  let other_view = f2.get_view("v2", uid1).unwrap();
  assert_eq!(view, other_view);
}
