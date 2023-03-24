use crate::core::BelongingsArray;

use collab::preclude::{Array, MapRefWrapper, ReadTxn, TransactionMut};

pub struct WorkspaceMap {
    root: MapRefWrapper,
    belongings: BelongingsArray,
}

const NAME: &str = "name";

impl WorkspaceMap {
    pub fn new(root: MapRefWrapper) -> Self {
        let belongings = BelongingsArray::new(&root);
        Self { root, belongings }
    }

    pub fn set_name_with_txn(&self, txn: &mut TransactionMut, name: &str) {
        self.root.insert_with_txn(txn, NAME, name);
    }
}
