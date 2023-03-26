use crate::util::{create_folder_with_workspace, make_test_view};

#[test]
fn create_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_view = make_test_view("v1", "w1", vec![]);
    folder_test.views.insert_view(o_view.clone());

    let r_view = folder_test.views.get_view("v1").unwrap();
    assert_eq!(o_view.name, r_view.name);
    assert_eq!(o_view.bid, r_view.bid);
    assert_eq!(o_view.belongings, r_view.belongings);
}

#[test]
fn create_view_with_sub_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let child_view = make_test_view("v1_1", "v1", vec![]);
    let view = make_test_view("v1", "w1", vec![child_view.id.clone()]);

    folder_test.views.insert_view(child_view.clone());
    folder_test.views.insert_view(view.clone());

    let r_view = folder_test.views.get_view("v1").unwrap();
    assert_eq!(view.name, r_view.name);
    assert_eq!(view.bid, r_view.bid);
    assert_eq!(view.belongings, r_view.belongings);

    let r_sub_view = folder_test.views.get_view(&r_view.belongings[0]).unwrap();
    assert_eq!(child_view.name, r_sub_view.name);
    assert_eq!(child_view.bid, r_sub_view.bid);
}

#[test]
fn delete_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let view_1 = make_test_view("v1", "w1", vec![]);
    let view_2 = make_test_view("v2", "w1", vec![]);
    let view_3 = make_test_view("v3", "w1", vec![]);
    folder_test.views.insert_view(view_1);
    folder_test.views.insert_view(view_2);
    folder_test.views.insert_view(view_3);

    let views = folder_test.views.get_views(&["v1", "v2", "v3"]);
    assert_eq!(views[0].id, "v1");
    assert_eq!(views[1].id, "v2");
    assert_eq!(views[2].id, "v3");

    folder_test.views.delete_views(vec!["v1", "v2", "v3"]);

    let views = folder_test.views.get_views(&["v1", "v2", "v3"]);
    assert_eq!(views.len(), 0);
}

#[test]
fn update_view_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let o_view = make_test_view("v1", "w1", vec![]);
    folder_test.views.insert_view(o_view);
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
