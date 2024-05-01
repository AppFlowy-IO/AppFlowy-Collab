use crate::preclude::{CollabContext, YrsDelta};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use yrs::types::text::{TextEvent, YChange};
use yrs::types::{Attrs, Delta};
use yrs::{Any, ReadTxn, Subscription, Text, TextRef, Transaction, TransactionMut};
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

  pub fn apply_delta_with_txn(&self, txn: &mut TransactionMut, delta: Vec<Delta>) {
    let mut index = 0;
    for d in delta {
      match d {
        Delta::Inserted(content, attrs) => {
          let value = content.to_string(txn);
          // don't use value.len() because it's not represent the unicode graphemes
          // for example, "a̐" has 1 grapheme but value.len() is 3, "中文" has 2 graphemes but value.len() is 6
          let len = value.graphemes(true).count() as u32;
          if let Some(attrs) = attrs {
            self
              .text_ref
              .insert_with_attributes(txn, index, &value, *attrs)
          } else {
            // TODO: This is a hack to get around the fact that Yrs doesn't
            // By setting empty attributes, prevent it from encountering a bug where it gets appended to the previous op.
            let attrs = Attrs::from([(Arc::from(""), Any::Null)]);
            self
              .text_ref
              .insert_with_attributes(txn, index, &value, attrs);
          }

          index += len;
        },
        Delta::Deleted(len) => {
          self.text_ref.remove_range(txn, index, len);
        },
        Delta::Retain(len, attrs) => {
          attrs.map(|attrs| {
            self.text_ref.format(txn, index, len, *attrs);
            Some(())
          });
          index += len;
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
