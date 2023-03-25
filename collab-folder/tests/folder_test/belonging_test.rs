use crate::util::create_folder_with_workspace;
use collab_folder::core::{Belongings, TrashItem, View, ViewLayout};

#[test]
fn create_belongings_test() {
    let folder_test = create_folder_with_workspace("1", "w1");

    let view_1_1 = make_test_view("1_1", "1", vec![]);
    let view_1_2 = make_test_view("1_2", "1", vec!["1_2_1".to_string(), "1_2_2".to_string()]);
    let view_1_2_1 = make_test_view("1_2_1", "1_2", vec![]);
    let view_1_2_2 = make_test_view("1_2_2", "1_2", vec![]);
    let view_1_3 = make_test_view("1_3", "1", vec![]);

    let view_1 = View {
        id: "1".to_string(),
        bid: "w1".to_string(),
        name: "".to_string(),
        desc: "".to_string(),
        belongings: Belongings::new(vec![
            "1_1".to_string(),
            "1_2".to_string(),
            "1_3".to_string(),
        ]),
        created_at: 0,
        layout: ViewLayout::Document,
    };

    folder_test.views.insert_view(view_1.clone());
    folder_test.views.insert_view(view_1_1);
    folder_test.views.insert_view(view_1_2.clone());
    folder_test.views.insert_view(view_1_2_1);
    folder_test.views.insert_view(view_1_2_2);
    folder_test.views.insert_view(view_1_3);

    let belongings = folder_test
        .belongings
        .get_belongings_array(&view_1.id)
        .unwrap()
        .get_belongings();
    assert_eq!(belongings.len(), 3);

    let belongings = folder_test
        .belongings
        .get_belongings_array(&view_1_2.id)
        .unwrap()
        .get_belongings();
    assert_eq!(belongings.len(), 2);
}

fn make_test_view(view_id: &str, bid: &str, belongings: Vec<String>) -> View {
    View {
        id: view_id.to_string(),
        bid: bid.to_string(),
        name: "".to_string(),
        desc: "".to_string(),
        belongings: Belongings::new(belongings),
        created_at: 0,
        layout: ViewLayout::Document,
    }
}
#[test]
fn move_belongings_test() {
    let folder_test = create_folder_with_workspace("1", "w1");
    let view_1_1 = make_test_view("1_1", "1", vec![]);
    let view_1_2 = make_test_view("1_2", "1", vec![]);
    let view_1_3 = make_test_view("1_3", "1", vec![]);

    let view_1 = View {
        id: "1".to_string(),
        bid: "w1".to_string(),
        name: "".to_string(),
        desc: "".to_string(),
        belongings: Belongings::new(vec![
            "1_1".to_string(),
            "1_2".to_string(),
            "1_3".to_string(),
        ]),
        created_at: 0,
        layout: ViewLayout::Document,
    };

    folder_test.views.insert_view(view_1.clone());
    folder_test.views.insert_view(view_1_1);
    folder_test.views.insert_view(view_1_2);
    folder_test.views.insert_view(view_1_3);

    let belonging_array = folder_test
        .belongings
        .get_belongings_array(&view_1.id)
        .unwrap();
    let belongings = belonging_array.get_belongings();
    assert_eq!(belongings[0], "1_1");
    assert_eq!(belongings[1], "1_2");
    assert_eq!(belongings[2], "1_3");

    // 1_1, 1_3
    belonging_array.move_belonging(2, 1);

    // 1_3
    // belonging_array.move_belonging(2, 0);
}
