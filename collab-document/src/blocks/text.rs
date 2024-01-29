use crate::blocks::text_entities::TextDelta;
use collab::preclude::*;
use std::collections::HashMap;

pub struct TextOperation {
  root: MapRefWrapper,
}

impl TextOperation {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  /// get text ref wrapper with text_id
  pub fn get_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str) -> TextRefWrapper {
    self
      .root
      .get_text_ref_with_txn(txn, text_id)
      .unwrap_or_else(|| self.create_text_with_txn(txn, text_id))
  }

  /// create text ref wrapper with text_id
  pub fn create_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str) -> TextRefWrapper {
    self.root.insert_text_with_txn(txn, text_id)
  }

  /// delete text ref wrapper with text_id
  pub fn delete_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str) {
    self.root.delete_with_txn(txn, text_id);
  }

  /// get text delta with text_id
  pub fn get_delta_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Option<Vec<TextDelta>> {
    let text_ref = self.root.get_text_ref_with_txn(txn, text_id)?;
    Some(
      text_ref
        .get_delta_with_txn(txn)
        .iter()
        .map(|d| TextDelta::from(txn, d.to_owned()))
        .collect(),
    )
  }

  /// apply text delta with text_id
  pub fn apply_delta_with_txn(
    &self,
    txn: &mut TransactionMut,
    text_id: &str,
    delta: Vec<TextDelta>,
  ) {
    let text_ref = self.get_text_with_txn(txn, text_id);
    let delta = delta.iter().map(|d| d.to_owned().to_delta()).collect();
    text_ref.apply_delta_with_txn(txn, delta);
  }

  /// get all text delta and serialize to json string
  pub fn serialize_all_text_delta(&self) -> HashMap<String, String> {
    let txn = self.root.transact();
    self
      .root
      .iter(&txn)
      .filter_map(|(k, _)| {
        self.get_delta_with_txn(&txn, k).map(|delta| {
          (
            k.to_string(),
            serde_json::to_string(&delta).unwrap_or_default(),
          )
        })
      })
      .collect()
  }

  /// get all text delta and join as string
  pub fn stringify_all_text_delta(&self) -> HashMap<String, String> {
    let txn = self.root.transact();
    self
      .root
      .iter(&txn)
      .filter_map(|(k, _)| {
        self.get_delta_with_txn(&txn, k).map(|delta| {
          let text: Vec<String> = delta
            .iter()
            .filter_map(|d| match d {
              TextDelta::Inserted(s, _) => Some(s.clone()),
              _ => None,
            })
            .collect();
          let text = text.join("");
          (k.to_string(), text)
        })
      })
      .collect()
  }
}
