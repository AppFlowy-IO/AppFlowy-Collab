use collab::{core::text_wrapper::TextDelta, preclude::*};
use serde_json::Value::Null;

pub struct TextMap {
    pub root: MapRefWrapper,
}

impl TextMap {
    pub fn new(root: MapRefWrapper) -> Self {
        Self { root }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let txn = self.root.transact();
        let mut obj = serde_json::json!({});

        self.root.iter(&txn).for_each(|(k, _)| {
            let mut delta_arr = vec![];
            self.get_delta_with_txn(&txn, k).iter().for_each(|delta| {
                let mut delta_item = serde_json::json!({});
                match delta {
                    Delta::Inserted(content, attrs) => {
                        delta_item["insert"] = serde_json::json!(content.to_string());
                        delta_item["attributes"] = match attrs {
                            Some(attrs) => {
                                let mut attrs_obj = serde_json::json!({});
                                attrs.iter().for_each(|(k, v)| {
                                    attrs_obj[k.to_string()] = v.to_string().parse().unwrap();
                                });
                                attrs_obj
                            }
                            None => Null,
                        }
                    }
                    _ => (),
                };
                delta_arr.push(delta_item);
            });
            obj[k.to_string()] = serde_json::json!(delta_arr);
        });
        obj
    }

    pub fn create_text(&self, txn: &mut TransactionMut, text_id: &str) -> TextRefWrapper {
        let text_map = self.root.insert_text_with_txn(txn, text_id);
        text_map
    }

    pub fn get_text(&self, text_id: &str) -> Option<TextRefWrapper> {
        let txn = self.root.transact();
        let text_map = self.root.get_text_ref_with_txn(&txn, text_id);
        text_map
    }

    pub fn apply_text_delta_with_txn(
        &self,
        txn: &mut TransactionMut,
        text_id: &str,
        delta: Vec<TextDelta>,
    ) {
        let text_ref = self
            .get_text(text_id)
            .unwrap_or_else(|| self.create_text(txn, text_id));
        text_ref.apply_delta_with_txn(txn, delta);
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

    pub fn delete_with_txn(&self, txn: &mut TransactionMut, text_id: &str) {
        self.root.delete_with_txn(txn, text_id);
    }
}
