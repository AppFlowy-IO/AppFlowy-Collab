use crate::util::create_folder_with_workspace;
use collab_folder::core::{Belongings, View, ViewLayout};

#[test]
fn create_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_view = View {
        id: "v1".to_string(),
        bid: Some("w1".to_string()),
        name: "My first view".to_string(),
        desc: "".to_string(),
        belongings: Default::default(),
        created_at: 0,
        layout: ViewLayout::Document,
    };
    folder_test.views.insert_view(o_view.clone());

    let r_view = folder_test.views.get_view("v1").unwrap();
    assert_eq!(o_view.name, r_view.name);
    assert_eq!(o_view.bid, r_view.bid);
    assert_eq!(o_view.belongings, r_view.belongings);
}

#[test]
fn create_view_with_sub_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_sub_view = View {
        id: "v1_1".to_string(),
        bid: Some("v1".to_string()),
        name: "My first sub view".to_string(),
        desc: "".to_string(),
        belongings: Default::default(),
        created_at: 0,
        layout: ViewLayout::Document,
    };

    let o_view = View {
        id: "v1".to_string(),
        bid: Some("w1".to_string()),
        name: "My first view".to_string(),
        desc: "".to_string(),
        belongings: Belongings::new(vec!["v1_1".to_string()]),
        created_at: 0,
        layout: ViewLayout::Document,
    };
    folder_test.views.insert_view(o_sub_view.clone());
    folder_test.views.insert_view(o_view.clone());

    let r_view = folder_test.views.get_view("v1").unwrap();
    assert_eq!(o_view.name, r_view.name);
    assert_eq!(o_view.bid, r_view.bid);
    assert_eq!(o_view.belongings, r_view.belongings);

    let r_sub_view = folder_test.views.get_view(&r_view.belongings[0]).unwrap();
    assert_eq!(o_sub_view.name, r_sub_view.name);
    assert_eq!(o_sub_view.bid, r_sub_view.bid);
}

#[test]
fn delete_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_view = View {
        id: "v1".to_string(),
        bid: Some("w1".to_string()),
        name: "My first view".to_string(),
        desc: "".to_string(),
        belongings: Default::default(),
        created_at: 0,
        layout: ViewLayout::Document,
    };
    folder_test.views.insert_view(o_view.clone());
    assert!(folder_test.views.get_view("v1").is_some());
    folder_test.views.delete_view("v1");
    assert!(folder_test.views.get_view("v1").is_none());
}

#[test]
fn update_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_view = View {
        id: "v1".to_string(),
        bid: Some("w1".to_string()),
        name: "My first view".to_string(),
        desc: "".to_string(),
        belongings: Default::default(),
        created_at: 0,
        layout: ViewLayout::Document,
    };
    folder_test.views.insert_view(o_view);
    folder_test
        .views
        .update_view("v1", |update| {
            update.set_name("Untitled").set_desc("My first view");
        })
        .unwrap();

    let r_view = folder_test.views.get_view("v1").unwrap();
    assert_eq!(r_view.name, "Untitled");
    assert_eq!(r_view.desc, "My first view");
}
