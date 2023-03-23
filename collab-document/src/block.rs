use collab::preclude::{CustomMapRef, MapRefWrapper, Prelim, TransactionMut};
use collab_derive::Collab;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Collab, Serialize, Deserialize)]
pub struct Block {
    pub id: String,

    #[serde(rename = "type")]
    pub ty: String,

    pub next: String,

    #[serde(rename = "firstChild")]
    pub first_child: String,

    pub data: String,
}

pub struct BlockBuilder<'a, 'b> {
    block_map: BlockMapRef,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> BlockBuilder<'a, 'b> {
    pub fn new(txn: &'a mut TransactionMut<'b>, container: &MapRefWrapper) -> Self {
        let key = nanoid!(4);
        Self::new_with_txn(txn, key, container)
    }

    pub fn new_with_txn(
        txn: &'a mut TransactionMut<'b>,
        block_id: String,
        container: &MapRefWrapper,
    ) -> Self {
        let map_ref = match container.get_map_with_txn(txn, &block_id) {
            None => container.create_map_with_txn(txn, &block_id),
            Some(map) => map,
        };
        let block_map = BlockMapRef::from_map_ref(map_ref);

        Self { block_map, txn }
    }

    pub fn with_type<T: AsRef<str>>(mut self, ty: T) -> Self {
        self.block_map.set_ty(self.txn, ty.as_ref().to_string());
        self
    }

    pub fn with_data<T: AsRef<str>>(mut self, data: T) -> Self {
        self.block_map.set_data(self.txn, data.as_ref().to_string());
        self
    }

    pub fn with_next<T: AsRef<str>>(mut self, next: T) -> Self {
        self.block_map.set_next(self.txn, next.as_ref().to_string());
        self
    }

    pub fn with_child<T: AsRef<str>>(mut self, child: T) -> Self {
        self.block_map
            .set_first_child(self.txn, child.as_ref().to_string());
        self
    }

    pub fn build(self) -> BlockMapRef {
        self.block_map
    }
}
