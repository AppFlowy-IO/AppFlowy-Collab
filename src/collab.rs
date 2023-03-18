use crate::entities::MapModifier;
use crate::util::{collaborate_json_object, print_map};
use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::de::Unexpected::Str;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use yrs::block::Prelim;
use yrs::types::Value::{Any, YMap};
use yrs::types::{Event, ToJson, Value};
use yrs::{
    Doc, Map, MapPrelim, MapRef, Observable, Subscription, Transact, Transaction, TransactionMut,
};

type SubscriptionCallback = Arc<dyn Fn(&TransactionMut, &Event) -> ()>;
type InnerSubscription = Subscription<SubscriptionCallback>;

pub struct Collab {
    id: String,
    doc: Doc,
    attributes: MapRef,
    subscription: Option<InnerSubscription>,
}

impl Collab {
    pub fn new(id: String) -> Collab {
        let doc = Doc::new();
        let attributes = doc.get_or_insert_map("attrs");
        Self {
            id,
            doc,
            attributes,
            subscription: None,
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let txn = self.doc.transact();
        self.attributes.get(&txn, &key)
    }

    pub fn insert<V: Prelim>(&self, key: &str, value: V) {
        let mut txn = self.doc.transact_mut();
        self.attributes.insert(&mut txn, key, value);
    }

    pub fn insert_object_with_path<T: Serialize>(
        &mut self,
        path: Vec<String>,
        id: &str,
        object: T,
    ) {
        let map = if path.is_empty() {
            None
        } else {
            let txn = self.transact();
            self.get_map_with_txn(&txn, path).map(|m| m.into_inner())
        };

        let mut txn = self.transact_mut();
        let value = serde_json::to_value(&object).unwrap();
        collaborate_json_object(id, &value, map, &mut txn, self);
    }

    pub fn get_object_with_path<T: DeserializeOwned>(
        &self,
        paths: Vec<String>,
    ) -> Option<(T, MapModifier)> {
        if paths.is_empty() {
            return None;
        }
        let txn = self.transact();
        let map = self.get_map_with_txn(&txn, paths)?;
        drop(txn);

        let json_str = map.to_json();
        let object = serde_json::from_str::<T>(&json_str).ok()?;
        return Some((object, map));
    }

    pub fn get_map_with_path(&self, paths: Vec<String>) -> Option<MapModifier> {
        let txn = self.doc.transact();
        self.get_map_with_txn(&txn, paths)
    }

    pub fn get_map_with_txn(&self, txn: &Transaction, paths: Vec<String>) -> Option<MapModifier> {
        if paths.is_empty() {
            return None;
        }
        let mut iter = paths.into_iter();
        let mut map = self.attributes.get(txn, &iter.next().unwrap())?.to_ymap();
        while let Some(path) = iter.next() {
            map = map?.get(txn, &path)?.to_ymap();
        }
        map.map(|m| MapModifier::new(m, self.doc.clone()))
    }

    pub fn create_map(&self, id: &str) -> MapRef {
        let mut txn = self.doc.transact_mut();
        self.create_map_with_transaction(id, &mut txn)
    }

    pub fn create_map_with_transaction(&self, id: &str, txn: &mut TransactionMut) -> MapRef {
        let map = MapPrelim::<lib0::any::Any>::new();
        self.attributes.insert(txn, id, map)
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        let txn = self.doc.transact();
        self.attributes
            .get(&txn, &key)
            .map(|val| val.to_string(&txn))
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let mut txn = self.doc.transact_mut();
        self.attributes.remove(&mut txn, key)
    }

    pub fn to_json(&self, txn: &Transaction) -> lib0::any::Any {
        self.attributes.to_json(txn)
    }
}

impl Display for Collab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())?;
        Ok(())
    }
}

impl std::ops::Deref for Collab {
    type Target = Doc;

    fn deref(&self) -> &Self::Target {
        &self.doc
    }
}

impl std::ops::DerefMut for Collab {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.doc
    }
}

#[cfg(test)]
mod tests {
    use crate::collab::Collab;
    use crate::util::collaborate_json_object;
    use serde::{Deserialize, Serialize};
    use yrs::types::ToJson;
    use yrs::{Map, Observable, Transact};

    #[test]
    fn insert_text() {
        let mut collab = Collab::new("1".to_string());
        let sub = collab.attributes.observe(|txn, event| {
            event.target().iter(txn).for_each(|(a, b)| {
                println!("{}: {}", a, b);
            });
        });

        collab.insert("text", "hello world");
        let value = collab.get_str("text");
        assert_eq!(value.unwrap(), "hello world".to_string());
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Person {
        name: String,
        position: Position,
    }

    #[derive(Default, Debug, Serialize, Deserialize)]
    struct Position {
        title: String,
        level: u8,
    }

    #[test]
    fn insert_json_object() {
        let mut collab = Collab::new("1".to_string());
        let object = Person {
            name: "nathan".to_string(),
            position: Position {
                title: "develop".to_string(),
                level: 3,
            },
        };
        collab.insert_object_with_path(vec!["person".to_string()], "person", object);
        println!("{}", collab);

        let (person, map) = collab
            .get_object_with_path::<Person>(vec!["person".to_string()])
            .unwrap();

        println!("{:?}", person);

        let (pos, map) = collab
            .get_object_with_path::<Position>(vec!["person".to_string(), "position".to_string()])
            .unwrap();
        println!("{:?}", pos);
    }

    #[test]
    fn mut_json_object() {
        let mut collab = Collab::new("1".to_string());
        let object = Person {
            name: "nathan".to_string(),
            position: Position {
                title: "developer".to_string(),
                level: 3,
            },
        };
        collab.insert_object_with_path(vec![], "person", object);
        collab
            .get_object_with_path::<Position>(vec!["person".to_string(), "position".to_string()])
            .unwrap()
            .1
            .insert("title", "manager");

        let title = collab
            .get_map_with_path(vec!["person".to_string(), "position".to_string()])
            .unwrap()
            .get_str("title")
            .unwrap();
        assert_eq!(title, "manager")
    }

    #[test]
    fn observer_object_mut() {
        let mut collab = Collab::new("1".to_string());
        let object = Person {
            name: "nathan".to_string(),
            position: Position {
                title: "developer".to_string(),
                level: 3,
            },
        };
        collab.insert_object_with_path(vec![], "person", object);
        let sub = collab
            .get_object_with_path::<Position>(vec!["person".to_string(), "position".to_string()])
            .unwrap()
            .1
            .observe(|txn, event| {
                event.target().iter(txn).for_each(|(a, b)| {
                    println!("{}: {}", a, b);
                });
            });

        let mut map = collab
            .get_map_with_path(vec!["person".to_string(), "position".to_string()])
            .unwrap();

        map.insert("title", "manager");
    }

    #[test]
    fn insert_map() {
        let mut collab = Collab::new("1".to_string());
        let c = collab.attributes.observe(|txn, event| {
            event.target().iter(txn).for_each(|(a, b)| {
                println!("{}: {}", a, b);
            });
        });

        let mut map = collab.create_map("map_object");
        let mut txn = collab.doc.transact_mut();
        map.insert(&mut txn, "a", "a text");
        map.insert(&mut txn, "b", "b text");
        map.insert(&mut txn, "c", 123);
        map.insert(&mut txn, "d", true);
        drop(txn);

        let value = collab
            .get_map_with_path(vec!["map_object".to_string()])
            .unwrap();
        let txn = collab.transact();
        value.iter(&txn).for_each(|(a, b)| {
            println!("{}:{}", a, b);
        });
    }
}
