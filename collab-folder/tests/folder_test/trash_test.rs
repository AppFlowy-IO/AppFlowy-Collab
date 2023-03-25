use crate::util::create_folder_with_workspace;
use collab_folder::core::TrashItem;

#[test]
fn create_trash_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    folder_test.trash.add_trash(TrashItem {
        id: "1".to_string(),
        created_at: 0,
    });

    folder_test.trash.add_trash(TrashItem {
        id: "2".to_string(),
        created_at: 0,
    });

    folder_test.trash.add_trash(TrashItem {
        id: "3".to_string(),
        created_at: 0,
    });

    let trash = folder_test.trash.get_all_trash();
    assert_eq!(trash.len(), 3);
    assert_eq!(trash[0].id, "1");
    assert_eq!(trash[1].id, "2");
    assert_eq!(trash[2].id, "3");
}

#[test]
fn delete_trash_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    folder_test.trash.add_trash(TrashItem {
        id: "1".to_string(),
        created_at: 0,
    });

    folder_test.trash.add_trash(TrashItem {
        id: "2".to_string(),
        created_at: 0,
    });

    let trash = folder_test.trash.get_all_trash();
    assert_eq!(trash[0].id, "1");
    assert_eq!(trash[1].id, "2");

    folder_test.trash.remove_trash("1");
    let trash = folder_test.trash.get_all_trash();
    assert_eq!(trash[0].id, "2");
}
