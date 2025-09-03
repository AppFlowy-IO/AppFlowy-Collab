use crate::util::{create_folder, make_test_view, parse_view_id};
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{Collab, Update};
use collab_folder::{Folder, UserId};
use uuid::Uuid;

#[test]
fn replace_view_get_view() {
  let uid = 1i64;
  let mut folder = create_folder(UserId::from(uid), "fake_workspace_id");
  let workspace_id = folder.get_workspace_id().unwrap();

  // Create initial views
  let v1 = make_test_view("v1", &workspace_id, vec![]);
  let v21 = make_test_view("v2.1", &workspace_id, vec![]);
  let v22 = make_test_view("v2.2", &workspace_id, vec![]);
  let v2 = make_test_view(
    "v2",
    &workspace_id,
    vec![
      crate::util::test_uuid("v2.1"),
      crate::util::test_uuid("v2.2"),
    ],
  );
  folder.insert_view(v1, None, uid);
  folder.insert_view(v21, None, uid);
  folder.insert_view(v22, None, uid);
  folder.insert_view(v2, None, uid);

  let v2_id = crate::util::test_uuid("v2").to_string();
  let old = folder.get_view(&parse_view_id(&v2_id), uid).unwrap();
  assert_eq!(
    old.id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2".as_bytes()).to_string()
  );

  folder.replace_view(
    &crate::util::test_uuid("v2"),
    &crate::util::test_uuid("v3"),
    uid,
  );

  // getting old view id should return new one
  let new = folder.get_view(&parse_view_id(&v2_id), uid).unwrap();
  assert_eq!(
    new.id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v3".as_bytes()).to_string()
  );
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
  let v2 = make_test_view(
    "v2",
    &workspace_id,
    vec![
      crate::util::test_uuid("v2.1"),
      crate::util::test_uuid("v2.2"),
    ],
  );
  f1.insert_view(v1, None, uid1);
  f1.insert_view(v21, None, uid1);
  f1.insert_view(v22, None, uid1);
  f1.insert_view(v2, None, uid1);

  let mut collab = Collab::new(uid2, Uuid::new_v4(), "device-2", 2);

  // sync initial state between f1 and f2
  collab
    .apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  let mut f2 = Folder::open(collab, None).unwrap();

  assert_eq!(f1.to_json_value(), f2.to_json_value());

  // concurrently replace view in f1 and add new child view in f2
  f1.replace_view(
    &crate::util::test_uuid("v2"),
    &crate::util::test_uuid("v3"),
    uid1,
  );

  let mut v23 = make_test_view("v2.3", &workspace_id, vec![]);
  v23.parent_view_id = Some(collab_entity::uuid_validation::view_id_from_any_string(
    "v2",
  ));
  f2.insert_view(v23, None, uid2);

  // cross-sync state between f1 and f2
  f2.apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  f1.apply_update(Update::decode_v2(&f2.encode_collab_v2().doc_state).unwrap())
    .unwrap();

  let v2_id = crate::util::test_uuid("v2").to_string();
  let v1 = f1.get_view(&parse_view_id(&v2_id), uid2).unwrap();
  assert_eq!(
    v1.id.to_string(),
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v3".as_bytes()).to_string()
  );
  assert_eq!(
    v1.children
      .iter()
      .map(|c| c.id.to_string())
      .collect::<Vec<_>>(),
    vec![
      uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2.1".as_bytes()).to_string(),
      uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2.2".as_bytes()).to_string(),
      uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "v2.3".as_bytes()).to_string()
    ]
  );

  let v2 = f2.get_view(&parse_view_id(&v2_id), uid1).unwrap();
  assert_eq!(v1, v2);
}

#[test]
fn replace_view_all_views_concurrent_update() {
  let uid1 = 1i64;
  let uid2 = 1i64;
  let mut f1 = create_folder(UserId::from(uid1), "fake_workspace_id");
  let workspace_id = f1.get_workspace_id().unwrap();

  // Create initial views
  let v1 = make_test_view("v1", &workspace_id, vec![]);
  let v21 = make_test_view("v2.1", &workspace_id, vec![]);
  let v22 = make_test_view("v2.2", &workspace_id, vec![]);
  let v2 = make_test_view(
    "v2",
    &workspace_id,
    vec![
      crate::util::test_uuid("v2.1"),
      crate::util::test_uuid("v2.2"),
    ],
  );
  f1.insert_view(v1, None, uid1);
  f1.insert_view(v21, None, uid1);
  f1.insert_view(v22, None, uid1);
  f1.insert_view(v2, None, uid1);

  let mut collab = Collab::new(uid2, Uuid::new_v4(), "device-2", 2);

  // sync initial state between f1 and f2
  collab
    .apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  let mut f2 = Folder::open(collab, None).unwrap();

  assert_eq!(f1.to_json_value(), f2.to_json_value());

  // concurrently replace view in f1 and add new child view in f2
  f1.replace_view(
    &crate::util::test_uuid("v2"),
    &crate::util::test_uuid("v3"),
    uid1,
  );

  let mut v23 = make_test_view("v2.3", &workspace_id, vec![]);
  v23.parent_view_id = Some(collab_entity::uuid_validation::view_id_from_any_string(
    "v2",
  ));
  f2.insert_view(v23, None, uid2);

  // cross-sync state between f1 and f2
  f2.apply_update(Update::decode_v2(&f1.encode_collab_v2().doc_state).unwrap())
    .unwrap();
  f1.apply_update(Update::decode_v2(&f2.encode_collab_v2().doc_state).unwrap())
    .unwrap();

  // check if both sides have the same views
  assert_eq!(f1.to_json_value(), f2.to_json_value());

  let mut v1 = f1.get_all_views(uid1);
  let mut v2 = f2.get_all_views(uid2);

  v1.sort_by_key(|v| v.id.to_string());
  v2.sort_by_key(|v| v.id.to_string());

  assert_eq!(v1, v2);
}
