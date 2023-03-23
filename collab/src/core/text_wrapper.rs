use crate::preclude::CollabContext;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use yrs::types::text::TextEvent;

use yrs::{Subscription, TextRef, Transaction, TransactionMut};

pub type TextSubscriptionCallback = Arc<dyn Fn(&TransactionMut, &TextEvent)>;
pub type TextSubscription = Subscription<TextSubscriptionCallback>;

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
