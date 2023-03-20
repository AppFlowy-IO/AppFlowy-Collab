use crate::helper::{make_collab_pair, Document, DocumentMapRef, Owner, OwnerMapRef};

#[test]
fn derive_string_test() {
    let (local, _remote, update_cache) = make_collab_pair();
    update_cache.clear();

    let mut map_ref = local
        .get_map_with_path::<DocumentMapRef>(vec!["document"])
        .unwrap();

    let name = map_ref.get_name(&local.transact()).unwrap();
    assert_eq!(name, "Hello world");

    local.with_transact_mut(|txn| {
        map_ref.set_name(txn, "Hello AppFlowy".to_string());
    });
    assert_eq!(update_cache.num_of_updates(), 1);

    let name = map_ref.get_name(&local.transact()).unwrap();
    assert_eq!(name, "Hello AppFlowy");
}

#[test]
fn derive_hash_map_test() {
    let (local, _remote, update_cache) = make_collab_pair();
    update_cache.clear();

    let mut map_ref = local
        .get_map_with_path::<DocumentMapRef>(vec!["document"])
        .unwrap();

    let attributes = map_ref.get_attributes(&local.transact()).unwrap();
    assert_eq!(attributes.get("1").unwrap(), "task 1");
    assert_eq!(attributes.get("2").unwrap(), "task 2");

    local.with_transact_mut(|txn| {
        map_ref.update_attributes_with_kv(txn, "1", "Hello AppFlowy".to_string());
    });

    assert_eq!(update_cache.num_of_updates(), 1);
    let mut attributes = map_ref.get_attributes(&local.transact()).unwrap();
    assert_eq!(attributes.get("1").unwrap(), "Hello AppFlowy");

    local.with_transact_mut(|txn| {
        attributes.insert("1".to_string(), "task 1".to_string());
        map_ref.set_attributes(txn, attributes);
    });
    assert_eq!(update_cache.num_of_updates(), 2);

    let attributes = map_ref.get_attributes(&local.transact()).unwrap();
    assert_eq!(attributes.get("1").unwrap(), "task 1");
}

#[test]
fn derive_json_value_test() {
    let (local, _remote, update_cache) = make_collab_pair();
    update_cache.clear();

    let mut map_ref = local
        .get_map_with_path::<OwnerMapRef>(vec!["document", "owner"])
        .unwrap();

    let name = map_ref.get_name(&local.transact()).unwrap();
    assert_eq!(name, "nathan".to_string());

    local.with_transact_mut(|txn| {
        map_ref.set_name(txn, "nathan.fu".to_string());
    });

    let owner = local
        .get_json_with_path::<Owner>(vec!["document", "owner"])
        .unwrap();
    assert_eq!(owner.name, "nathan.fu".to_string());
}
