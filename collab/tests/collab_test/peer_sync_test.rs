use crate::helper::make_collab_pair;
use collab::preclude::MapRefWrapper;

#[test]
fn sync_document_edit() {
    let old_email = "nathan@appflowy.io";
    let new_email = "nathan@gmail.com";

    let (local, remote, update_cache) = make_collab_pair();
    let path = vec!["document", "owner"];
    let mut map = local
        .get_map_with_path::<MapRefWrapper>(path.clone())
        .unwrap();
    map.insert("email", new_email);

    let email = remote
        .get_map_with_path::<MapRefWrapper>(path.clone())
        .unwrap()
        .get_str("email")
        .unwrap();
    assert_eq!(email, old_email);

    let update = update_cache.get_update().unwrap();
    remote.with_transact_mut(|txn| txn.apply_update(update));

    let email = remote
        .get_map_with_path::<MapRefWrapper>(path.clone())
        .unwrap()
        .get_str("email")
        .unwrap();
    assert_eq!(email, new_email);
}
