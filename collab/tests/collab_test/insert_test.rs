use crate::helper::{Person, Position};
use collab::MapRefWrapper;

use collab::core::collab::Collab;
use yrs::{Map, Observable};

#[test]
fn insert_text() {
    let mut collab = Collab::new(1, "1");
    let _sub = collab.observer_attrs(|txn, event| {
        event.target().iter(txn).for_each(|(a, b)| {
            println!("{}: {}", a, b);
        });
    });

    collab.insert("text", "hello world");
    let value = collab.get("text").unwrap();
    let s = value.to_string(&collab.transact());
    assert_eq!(s, "hello world".to_string());
}

#[test]
fn insert_json_attrs() {
    let mut collab = Collab::new(1, "1");
    let object = Person {
        name: "nathan".to_string(),
        position: Position {
            title: "develop".to_string(),
            level: 3,
        },
    };
    collab.insert_json_with_path(vec![], "person", object);
    println!("{}", collab);

    let person = collab
        .get_json_with_path::<Person>(vec!["person".to_string()])
        .unwrap();

    println!("{:?}", person);

    let pos = collab
        .get_json_with_path::<Position>(vec!["person".to_string(), "position".to_string()])
        .unwrap();
    println!("{:?}", pos);
}

#[test]
fn observer_attr_mut() {
    let mut collab = Collab::new(1, "1");
    let object = Person {
        name: "nathan".to_string(),
        position: Position {
            title: "developer".to_string(),
            level: 3,
        },
    };
    collab.insert_json_with_path(vec![], "person", object);
    let _sub = collab
        .get_map_with_path::<MapRefWrapper>(vec!["person".to_string(), "position".to_string()])
        .unwrap()
        .observe(|txn, event| {
            event.target().iter(txn).for_each(|(a, b)| {
                println!("{}: {}", a, b);
            });
        });

    let mut map = collab
        .get_map_with_path::<MapRefWrapper>(vec!["person".to_string(), "position".to_string()])
        .unwrap();

    map.insert("title", "manager");
}

#[test]
fn remove_value() {
    let mut collab = Collab::new(1, "1");
    let object = Person {
        name: "nathan".to_string(),
        position: Position {
            title: "developer".to_string(),
            level: 3,
        },
    };
    collab.insert_json_with_path(vec![], "person", object);
    let map = collab
        .get_map_with_path::<MapRefWrapper>(vec!["person".to_string(), "position".to_string()]);
    assert!(map.is_some());

    collab.remove_with_path(vec!["person".to_string(), "position".to_string()]);

    let map = collab
        .get_map_with_path::<MapRefWrapper>(vec!["person".to_string(), "position".to_string()]);
    assert!(map.is_none());
}
