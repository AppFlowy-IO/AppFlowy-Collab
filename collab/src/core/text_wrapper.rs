use crate::preclude::{CollabContext, YrsDelta};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use yrs::types::text::{TextEvent, YChange};
use yrs::types::Delta;
use yrs::{In, ReadTxn, Subscription, Text, TextRef, Transaction, TransactionMut};
pub type TextSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &TextEvent)>;
pub type TextSubscription = Subscription;

pub struct TextRefWrapper {
  text_ref: TextRef,
  collab_ctx: CollabContext,
}

impl TextRefWrapper {
  pub fn new(text_ref: TextRef, collab_ctx: CollabContext) -> Self {
    Self {
      text_ref,
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

  pub fn get_delta_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Delta> {
    let changes = self.text_ref.diff(txn, YChange::identity);
    let mut deltas = vec![];
    for change in changes {
      let delta = YrsDelta::Inserted(change.insert, change.attributes);
      deltas.push(delta);
    }
    deltas
  }

  pub fn apply_delta_with_txn(&self, txn: &mut TransactionMut, delta: Vec<Delta<In>>) {
    self.text_ref.apply_delta(txn, delta);
  }
}

impl Deref for TextRefWrapper {
  type Target = TextRef;

  fn deref(&self) -> &Self::Target {
    &self.text_ref
  }
}

impl DerefMut for TextRefWrapper {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.text_ref
  }
}
