use collab::core::text_wrapper::TextSubscription;
use collab::preclude::*;
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct TextMap {
    pub root: MapRefWrapper,
    pub subscriptions: RwLock<HashMap<String, TextSubscription>>,
}
impl TextMap {
    pub fn new(root: MapRefWrapper) -> Self {
        Self {
            root,
            subscriptions: Default::default(),
        }
    }

    pub fn create_text(&self, text_id: &str) -> TextRefWrapper {
        self.root.with_transact_mut(|txn| {
            let mut text_ref = self.root.insert_text_with_txn(txn, text_id);
            let subscription = text_ref.observe(|txn, event| {
                for delta in event.delta(txn) {
                    match delta {
                        YrsDelta::Inserted(_, _) => {}
                        YrsDelta::Deleted(_) => {}
                        YrsDelta::Retain(_, _) => {}
                    }
                }
            });
            self.subscriptions
                .write()
                .insert(text_id.to_string(), subscription);
            text_ref
        })
    }

    pub fn get_text(&self, text_id: &str) -> Option<TextRefWrapper> {
        let txn = self.root.transact();
        self.root.get_text_ref_with_txn(&txn, text_id)
    }

    pub fn edit_text(&self, text_id: &str, actions: Vec<TextAction>) {
        let text_ref = self
            .get_text(text_id)
            .unwrap_or_else(|| self.create_text(text_id));
        self.root
            .with_transact_mut(|txn| self.edit_text_with_txn(txn, &text_ref, actions))
    }

    pub fn get_str(&self, text_id: &str) -> Option<String> {
        let txn = self.root.transact();
        self.get_str_with_txn(&txn, text_id)
    }

    pub fn get_str_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Option<String> {
        self.root
            .get_text_ref_with_txn(txn, text_id)
            .map(|map| map.get_string(txn))
    }

    pub fn get_delta(&self, text_id: &str) -> Vec<YrsDelta> {
        let txn = self.root.transact();
        self.root
            .get_text_ref_with_txn(&txn, text_id)
            .map(|map| map.get_delta_with_txn(&txn))
            .unwrap_or_default()
    }

    pub fn get_delta_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Vec<YrsDelta> {
        self.root
            .get_text_ref_with_txn(txn, text_id)
            .map(|map| map.get_delta_with_txn(txn))
            .unwrap_or_default()
    }

    pub fn edit_text_with_txn(
        &self,
        txn: &mut TransactionMut,
        text_ref: &TextRefWrapper,
        actions: Vec<TextAction>,
    ) {
        self.apply_text_actions(txn, text_ref, actions);
    }

    fn apply_text_actions(
        &self,
        txn: &mut TransactionMut,
        text_ref: &TextRefWrapper,
        actions: Vec<TextAction>,
    ) {
        for action in actions {
            match action {
                TextAction::Insert { index, s, attrs } => match attrs {
                    None => text_ref.insert(txn, index, &s),
                    Some(attrs) => text_ref.insert_with_attributes(txn, index, &s, attrs),
                },
                TextAction::Remove { index, len } => text_ref.remove_range(txn, index, len),
                TextAction::Format { index, len, attrs } => text_ref.format(txn, index, len, attrs),
                TextAction::Push { s } => text_ref.push(txn, &s),
            }
        }
    }
}

pub enum TextAction {
    Insert {
        index: u32,
        s: String,
        attrs: Option<Attrs>,
    },
    Remove {
        index: u32,
        len: u32,
    },
    Format {
        index: u32,
        len: u32,
        attrs: Attrs,
    },
    Push {
        s: String,
    },
}
