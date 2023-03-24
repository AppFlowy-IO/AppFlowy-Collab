use crate::preclude::{CollabContext, YrsValue};
use crate::util::insert_json_value_to_array_ref;
use anyhow::Result;
use serde::Serialize;
use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::{Array, ArrayRef, Transaction, TransactionMut};

pub struct ArrayRefWrapper {
    array_ref: ArrayRef,
    collab_ctx: CollabContext,
}

impl ArrayRefWrapper {
    pub fn new(array_ref: ArrayRef, collab_ctx: CollabContext) -> Self {
        Self {
            array_ref,
            collab_ctx,
        }
    }

    pub fn transact(&self) -> Transaction {
        self.collab_ctx.transact()
    }

    pub fn with_transact_mut<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut TransactionMut) -> T,
    {
        self.collab_ctx.with_transact_mut(f)
    }

    pub fn push<V: Prelim>(&self, value: V) {
        self.with_transact_mut(|txn| {
            self.array_ref.push_back(txn, value);
        });
    }

    pub fn get(&self, index: u32) -> Option<YrsValue> {
        let txn = self.transact();
        self.array_ref.get(&txn, index)
    }

    pub fn push_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, value: V) {
        self.array_ref.push_back(txn, value);
    }

    pub fn push_json_with_txn<T: Serialize>(
        &self,
        txn: &mut TransactionMut,
        value: T,
    ) -> Result<()> {
        let value = serde_json::to_value(value)?;
        insert_json_value_to_array_ref(txn, &self.array_ref, &value);
        Ok(())
    }

    pub fn remove_with_txn(&self, txn: &mut TransactionMut, index: u32) {
        self.array_ref.remove(txn, index);
    }
}

impl Deref for ArrayRefWrapper {
    type Target = ArrayRef;

    fn deref(&self) -> &Self::Target {
        &self.array_ref
    }
}

impl DerefMut for ArrayRefWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.array_ref
    }
}
