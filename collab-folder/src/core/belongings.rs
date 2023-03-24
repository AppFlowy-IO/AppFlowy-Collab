use crate::core::Belongings;
use collab::preclude::{Array, ArrayRefWrapper, MapRefWrapper, ReadTxn, TransactionMut};

const BELONGINGS: &str = "belongings";
pub struct BelongingsArray {
    belongings: ArrayRefWrapper,
}

impl BelongingsArray {
    pub fn new(root: &MapRefWrapper) -> Self {
        let belongings = root
            .get_array_ref(BELONGINGS)
            .unwrap_or_else(|| root.insert_array(BELONGINGS, Belongings::new().into_inner()));
        Self { belongings }
    }

    pub fn from_array(belongings: ArrayRefWrapper) -> Self {
        Self { belongings }
    }

    pub fn get_belongings(&self) -> Belongings {
        let txn = self.belongings.transact();
        self.get_belongings_with_txn(&txn)
    }

    pub fn get_belongings_with_txn<T: ReadTxn>(&self, txn: &T) -> Belongings {
        let mut belongings = Belongings::new();
        for value in self.belongings.iter(txn) {
            belongings.view_ids.push(value.to_string(txn));
        }
        belongings
    }

    pub fn move_belonging_with_txn(&self, txn: &mut TransactionMut, from: u32, to: u32) {
        self.belongings.move_to(txn, from, to)
    }

    pub fn remove_belonging_with_txn(&self, txn: &mut TransactionMut, index: u32) {
        self.belongings.remove_with_txn(txn, index);
    }

    pub fn add_belonging_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
        self.belongings.push_with_txn(txn, view_id)
    }
}
