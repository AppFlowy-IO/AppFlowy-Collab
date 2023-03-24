use collab::preclude::{lib0Any, Array, ArrayRefWrapper, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};

pub struct TrashArray {
    container: ArrayRefWrapper,
}

impl TrashArray {
    pub fn new(root: ArrayRefWrapper) -> Self {
        Self { container: root }
    }

    pub fn get_all_trash(&self) -> Vec<TrashItem> {
        let txn = self.container.transact();
        self.get_all_trash_with_txn(&txn)
    }

    pub fn get_all_trash_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<TrashItem> {
        let mut trash = vec![];
        for value in self.container.iter(txn) {
            if let YrsValue::Any(any) = value {
                trash.push(TrashItem::from(any));
            }
        }
        trash
    }

    pub fn remove_trash(&self, index: u32) {
        self.container.with_transact_mut(|txn| {
            self.container.remove_with_txn(txn, index);
        })
    }

    pub fn remove_trash_with_txn(&self, txn: &mut TransactionMut, index: u32) {
        self.container.remove_with_txn(txn, index);
    }

    pub fn add_trash(&self, trash: TrashItem) {
        self.container.with_transact_mut(|txn| {
            self.container.push_with_txn(txn, trash);
        })
    }

    pub fn add_trash_with_txn(&self, txn: &mut TransactionMut, trash: TrashItem) {
        self.container.push_with_txn(txn, trash);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
}

impl From<lib0Any> for TrashItem {
    fn from(any: lib0Any) -> Self {
        let mut json = String::new();
        any.to_json(&mut json);
        serde_json::from_str(&json).unwrap()
    }
}

impl From<TrashItem> for lib0Any {
    fn from(item: TrashItem) -> Self {
        let json = serde_json::to_string(&item).unwrap();
        lib0Any::from_json(&json).unwrap()
    }
}
