use crate::preclude::{CollabContext, YrsDelta};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use yrs::types::text::{TextEvent, YChange};
use yrs::types::{Attrs, Delta};
use yrs::{ReadTxn, Subscription, Text, TextRef, Transaction, TransactionMut};
pub type TextSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &TextEvent)>;
pub type TextSubscription = Subscription<TextSubscriptionCallback>;

pub enum TextDelta {
  Inserted(String, Attrs),

  /// Determines a change that resulted in removing a consecutive range of characters.
  Deleted(u32),

  /// Determines a number of consecutive unchanged characters. Used to recognize non-edited spaces
  /// between [Delta::Inserted] and/or [Delta::Deleted] chunks. Can contain an optional set of
  /// attributes, which have been used to format an existing piece of text.
  Retain(u32, Attrs),
}

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

  pub fn apply_delta_with_txn(&self, txn: &mut TransactionMut, delta: Vec<TextDelta>) {
    let mut index = 0;
    for d in delta {
      match d {
        TextDelta::Inserted(content, attrs) => {
          let value = content.to_string();
          let len = value.len() as u32;
          self.text_ref.insert(txn, index, &value);
          self.text_ref.format(txn, index, len, attrs);
          index = index + len;
        },
        TextDelta::Deleted(len) => {
          self.text_ref.remove_range(txn, index, len);
          index = index + len;
        },
        TextDelta::Retain(len, attrs) => {
          self.text_ref.format(txn, index, len, attrs);
          index = index + len;
        },
      }
    }
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
